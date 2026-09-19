use std::{
    error::Error,
    ffi::OsString,
    fs,
    io::{self, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use tempfile::NamedTempFile;

use crate::{
    cli::{Cli, Mode},
    clipboard,
    config::{Config, EffectiveConfig},
    git::{detect_base, repository_name, resolved_diff_args},
};

type CopyResult = Result<(), Box<dyn Error>>;
type ClipboardWriter<'a> = dyn FnMut(&[u8], bool) -> CopyResult + 'a;

struct MultipleOutputs<'a> {
    environment: &'a Environment,
    effective: &'a EffectiveConfig,
    osc52_fallback: bool,
    plan: OutputPlan,
    stdout: &'a mut dyn Write,
    messages: &'a mut dyn Write,
    copy: &'a mut ClipboardWriter<'a>,
}

impl MultipleOutputs<'_> {
    fn write_file(&mut self, mode: &Mode, payload: &[u8], errors: &mut Vec<String>) {
        if self.plan.file {
            if let Err(error) = write_payload_to_file(
                mode,
                &self.environment.cwd,
                &self.environment.git_program,
                &self.effective.output_dir,
                self.effective.quiet || self.plan.stdout,
                self.messages,
                payload,
            ) {
                errors.push(error.to_string());
            }
        }
    }

    fn write_stdout(&mut self, payload: &[u8], errors: &mut Vec<String>) {
        if self.plan.stdout {
            if let Err(error) = self
                .stdout
                .write_all(payload)
                .and_then(|()| self.stdout.flush())
            {
                errors.push(format!("cannot write diff to stdout: {error}"));
            }
        }
    }

    fn write_clipboard(&mut self, mode: &Mode, payload: &[u8], errors: &mut Vec<String>) {
        let result = if payload.is_empty() && !self.plan.file {
            write_empty_message(
                mode,
                &self.environment.cwd,
                &self.environment.git_program,
                self.effective.quiet || self.plan.stdout,
                self.messages,
            )
        } else if self.plan.clipboard && !payload.is_empty() {
            (self.copy)(payload, self.osc52_fallback)
        } else {
            Ok(())
        };
        if let Err(error) = result {
            errors.push(error.to_string());
        }
    }
}

pub fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    run_in(cli, Environment::system()?)
}

pub struct Environment {
    pub cwd: PathBuf,
    pub config_path: Option<PathBuf>,
    pub git_program: OsString,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OutputPlan {
    stdout: bool,
    file: bool,
    clipboard: bool,
}

impl OutputPlan {
    const fn new(stdout: bool, file: bool, clipboard: bool) -> Self {
        Self {
            stdout,
            file,
            clipboard,
        }
    }
}

fn output_plan(cli: &Cli, stdout_is_terminal: bool) -> OutputPlan {
    OutputPlan {
        stdout: cli.stdout || !stdout_is_terminal,
        file: cli.output_dir.is_some()
            || (stdout_is_terminal && (!cli.stdout || cli.copy_save) && !cli.copy),
        clipboard: cli.copy || cli.copy_save,
    }
}

impl Environment {
    fn system() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            cwd: std::env::current_dir()?,
            config_path: Config::default_path().ok(),
            git_program: OsString::from("git"),
        })
    }
}

pub fn run_in(cli: Cli, environment: Environment) -> Result<(), Box<dyn Error>> {
    let plan = output_plan(&cli, io::stdout().is_terminal());
    if plan.stdout {
        return run_with_outputs(
            cli,
            environment,
            plan,
            &mut io::stdout().lock(),
            &mut io::sink(),
            &mut |payload, fallback| clipboard::copy(payload, fallback).map_err(Into::into),
        );
    }
    run_with_outputs(
        cli,
        environment,
        plan,
        &mut io::sink(),
        &mut io::stdout().lock(),
        &mut |payload, fallback| clipboard::copy(payload, fallback).map_err(Into::into),
    )
}

pub fn run_in_with_writer(
    cli: Cli,
    environment: Environment,
    messages: &mut dyn Write,
) -> Result<(), Box<dyn Error>> {
    let plan = output_plan(&cli, true);
    run_with_outputs(
        cli,
        environment,
        plan,
        &mut io::stdout().lock(),
        messages,
        &mut |payload, fallback| clipboard::copy(payload, fallback).map_err(Into::into),
    )
}

fn run_with_outputs(
    cli: Cli,
    environment: Environment,
    plan: OutputPlan,
    stdout: &mut dyn Write,
    messages: &mut dyn Write,
    copy: &mut ClipboardWriter<'_>,
) -> Result<(), Box<dyn Error>> {
    let mode = cli.mode()?;
    if cli.no_header {
        if let Some(result) = stdout_without_config(&mode, &environment, plan) {
            return result;
        }
    }

    let config = load_config(environment.config_path.as_deref())?;
    let effective = EffectiveConfig::new(&config, &cli, &environment.cwd);
    let base = resolve_base(&mode, config.base_branch, &environment)?;

    if !effective.header && plan == OutputPlan::new(true, false, false) {
        return run_to_stdout(
            &mode,
            base.as_deref(),
            &environment.cwd,
            &environment.git_program,
        );
    }
    if !effective.header && plan == OutputPlan::new(false, true, false) {
        return run_to_file(
            &mode,
            base.as_deref(),
            &environment.cwd,
            &environment.git_program,
            &effective.output_dir,
            effective.quiet,
            messages,
        );
    }

    run_to_multiple_outputs(
        &mode,
        base.as_deref(),
        MultipleOutputs {
            environment: &environment,
            effective: &effective,
            osc52_fallback: config.clipboard.osc52_fallback,
            plan,
            stdout,
            messages,
            copy,
        },
    )
}

fn load_config(path: Option<&Path>) -> Result<Config, Box<dyn Error>> {
    match path {
        Some(path) => Ok(Config::load(path)?),
        None => Ok(Config::default()),
    }
}

fn stdout_without_config(
    mode: &Mode,
    environment: &Environment,
    plan: OutputPlan,
) -> Option<Result<(), Box<dyn Error>>> {
    if plan != OutputPlan::new(true, false, false) {
        return None;
    }
    let base = match mode {
        Mode::Branch(None) => return None,
        Mode::Branch(Some(base)) => Some(base.as_str()),
        _ => None,
    };
    Some(run_to_stdout(
        mode,
        base,
        &environment.cwd,
        &environment.git_program,
    ))
}

fn resolve_base(
    mode: &Mode,
    configured: Option<String>,
    environment: &Environment,
) -> Result<Option<String>, Box<dyn Error>> {
    match mode {
        Mode::Branch(Some(explicit)) => Ok(Some(explicit.clone())),
        Mode::Branch(None) => {
            let base = match configured {
                Some(base) => base,
                None => detect_base(&environment.cwd, &environment.git_program)?,
            };
            Ok(Some(base))
        }
        _ => Ok(None),
    }
}

fn run_to_multiple_outputs(
    mode: &Mode,
    base: Option<&str>,
    mut outputs: MultipleOutputs<'_>,
) -> Result<(), Box<dyn Error>> {
    let mut payload = render_diff(
        mode,
        base,
        &outputs.environment.cwd,
        &outputs.environment.git_program,
    )?;
    if outputs.effective.header && !payload.is_empty() {
        payload = annotate_diff(
            mode,
            base,
            outputs.effective.note.as_deref(),
            &outputs.environment.cwd,
            &outputs.environment.git_program,
            payload,
        )?;
    }
    let mut errors = Vec::new();
    outputs.write_file(mode, &payload, &mut errors);
    outputs.write_stdout(&payload, &mut errors);
    outputs.write_clipboard(mode, &payload, &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; ").into())
    }
}

fn annotate_diff(
    mode: &Mode,
    base: Option<&str>,
    note: Option<&str>,
    cwd: &Path,
    git_program: &OsString,
    diff: Vec<u8>,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let repository = repository_name(cwd, git_program)?
        .replace('\r', "\\r")
        .replace('\n', "\\n");
    let mut payload = format!(
        "# contents: {}\n# repository: {repository}\n",
        mode.contents(base)
    )
    .into_bytes();
    if let Some(note) = note {
        if note.contains('\n') || note.contains('\r') {
            payload.extend_from_slice(b"# note:\n");
            let note = note.replace("\r\n", "\n").replace('\r', "\n");
            for line in note.split('\n') {
                if line.chars().all(char::is_whitespace) {
                    payload.extend_from_slice(b"#\n");
                } else {
                    payload.extend_from_slice(b"#   ");
                    payload.extend_from_slice(line.as_bytes());
                    payload.push(b'\n');
                }
            }
        } else {
            payload.extend_from_slice(format!("# note: {note}\n").as_bytes());
        }
    }
    payload.push(b'\n');
    payload.extend_from_slice(&diff);
    Ok(payload)
}

fn render_diff(
    mode: &Mode,
    base: Option<&str>,
    cwd: &Path,
    git_program: &OsString,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let output = Command::new(git_program)
        .args(resolved_diff_args(mode, base, cwd, git_program)?)
        .current_dir(cwd)
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("failed to start git: {error}"))?;
    if !output.status.success() {
        return Err(format!("git diff failed with {}", output.status).into());
    }
    Ok(output.stdout)
}

fn run_to_stdout(
    mode: &Mode,
    base: Option<&str>,
    cwd: &Path,
    git_program: &OsString,
) -> Result<(), Box<dyn Error>> {
    let status = Command::new(git_program)
        .args(resolved_diff_args(mode, base, cwd, git_program)?)
        .current_dir(cwd)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| format!("failed to start git: {error}"))?;
    if !status.success() {
        return Err(format!("git diff failed with {status}").into());
    }
    Ok(())
}

fn run_to_file(
    mode: &Mode,
    base: Option<&str>,
    cwd: &Path,
    git_program: &OsString,
    output_dir: &Path,
    quiet: bool,
    messages: &mut dyn Write,
) -> Result<(), Box<dyn Error>> {
    let temporary = create_output_file(output_dir)?;
    let patch_stdout = temporary.reopen()?;
    let status = Command::new(git_program)
        .args(resolved_diff_args(mode, base, cwd, git_program)?)
        .current_dir(cwd)
        .stdout(Stdio::from(patch_stdout))
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| format!("failed to start git: {error}"))?;

    if !status.success() {
        return Err(format!("git diff failed with {status}").into());
    }
    finish_file(
        mode,
        cwd,
        git_program,
        output_dir,
        quiet,
        messages,
        temporary,
    )
}

fn create_output_file(output_dir: &Path) -> Result<NamedTempFile, Box<dyn Error>> {
    fs::create_dir_all(output_dir).map_err(|error| {
        format!(
            "cannot create output directory '{}': {error}",
            output_dir.display()
        )
    })?;
    NamedTempFile::new_in(output_dir)
        .map_err(|error| {
            format!(
                "cannot create temporary diff in '{}': {error}",
                output_dir.display()
            )
        })
        .map_err(Into::into)
}

fn write_payload_to_file(
    mode: &Mode,
    cwd: &Path,
    git_program: &OsString,
    output_dir: &Path,
    quiet: bool,
    messages: &mut dyn Write,
    payload: &[u8],
) -> Result<(), Box<dyn Error>> {
    let mut temporary = create_output_file(output_dir)?;
    temporary.write_all(payload)?;
    finish_file(
        mode,
        cwd,
        git_program,
        output_dir,
        quiet,
        messages,
        temporary,
    )
}

fn finish_file(
    mode: &Mode,
    cwd: &Path,
    git_program: &OsString,
    output_dir: &Path,
    quiet: bool,
    messages: &mut dyn Write,
    temporary: NamedTempFile,
) -> Result<(), Box<dyn Error>> {
    let destination = output_dir.join(mode.filename());
    let bytes = temporary.as_file().metadata()?.len();
    if bytes == 0 {
        return finish_empty_file(
            mode,
            cwd,
            git_program,
            quiet,
            messages,
            temporary,
            &destination,
        );
    }

    temporary.persist(&destination).map_err(|error| {
        format!(
            "cannot replace diff '{}': {}",
            destination.display(),
            error.error
        )
    })?;
    if !quiet {
        writeln!(
            messages,
            "Wrote {} ({})",
            mode.filename(),
            format_size(bytes)
        )?;
    }
    Ok(())
}

fn finish_empty_file(
    mode: &Mode,
    cwd: &Path,
    git_program: &OsString,
    quiet: bool,
    messages: &mut dyn Write,
    temporary: NamedTempFile,
    destination: &Path,
) -> Result<(), Box<dyn Error>> {
    drop(temporary);
    match fs::remove_file(destination) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "cannot remove stale diff '{}': {error}",
                destination.display()
            )
            .into());
        }
    }
    write_empty_message(mode, cwd, git_program, quiet, messages)
}

fn write_empty_message(
    mode: &Mode,
    cwd: &Path,
    git_program: &OsString,
    quiet: bool,
    messages: &mut dyn Write,
) -> Result<(), Box<dyn Error>> {
    if quiet {
        return Ok(());
    }
    let untracked = match mode {
        Mode::Default | Mode::Unstaged | Mode::All => count_untracked(cwd, git_program)?,
        _ => 0,
    };
    writeln!(messages, "{}", empty_message(mode, untracked)).map_err(Into::into)
}

fn count_untracked(cwd: &Path, git_program: &OsString) -> Result<usize, Box<dyn Error>> {
    let mut child = Command::new(git_program)
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("failed to start git: {error}"))?;
    let mut stdout = child.stdout.take().expect("piped stdout is available");
    let mut buffer = [0; 8192];
    let mut count = 0;
    loop {
        let read = stdout.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        count += buffer[..read].iter().filter(|byte| **byte == 0).count();
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(format!("git ls-files failed with {status}").into());
    }
    Ok(count)
}

fn empty_message(mode: &Mode, untracked: usize) -> String {
    if untracked > 0 {
        let noun = if untracked == 1 { "file" } else { "files" };
        return match mode {
            Mode::Default | Mode::Unstaged => {
                format!("No unstaged tracked changes ({untracked} untracked {noun} not included).")
            }
            Mode::All => format!(
                "No uncommitted tracked changes ({untracked} untracked {noun} not included)."
            ),
            _ => unreachable!("untracked count is only requested for working-tree modes"),
        };
    }
    match mode {
        Mode::Default | Mode::Unstaged => "No unstaged changes.".into(),
        Mode::Staged => "No staged changes.".into(),
        Mode::All => "No uncommitted changes.".into(),
        Mode::Branch(_) => "No branch changes.".into(),
        Mode::Commits(1) => "No changes in the last commit.".into(),
        Mode::Commits(count) => format!("No changes in the last {count} commits."),
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    }
}

#[cfg(test)]
mod tests;
