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
    git::{detect_base, resolved_diff_args},
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
    if let Some(result) = stdout_without_config(&mode, &environment, plan) {
        return result;
    }

    let config = load_config(environment.config_path.as_deref())?;
    let effective = EffectiveConfig::new(&config, &cli, &environment.cwd);
    let base = resolve_base(&mode, config.base_branch, &environment)?;

    if plan == OutputPlan::new(true, false, false) {
        return run_to_stdout(
            &mode,
            base.as_deref(),
            &environment.cwd,
            &environment.git_program,
        );
    }
    if plan == OutputPlan::new(false, true, false) {
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
    outputs: MultipleOutputs<'_>,
) -> Result<(), Box<dyn Error>> {
    let payload = render_diff(
        mode,
        base,
        &outputs.environment.cwd,
        &outputs.environment.git_program,
    )?;
    let mut errors = Vec::new();
    if outputs.plan.file {
        if let Err(error) = write_payload_to_file(
            mode,
            &outputs.environment.cwd,
            &outputs.environment.git_program,
            &outputs.effective.output_dir,
            outputs.effective.quiet || outputs.plan.stdout,
            outputs.messages,
            &payload,
        ) {
            errors.push(error.to_string());
        }
    }
    if outputs.plan.stdout {
        if let Err(error) = outputs
            .stdout
            .write_all(&payload)
            .and_then(|()| outputs.stdout.flush())
        {
            errors.push(format!("cannot write diff to stdout: {error}"));
        }
    }
    if outputs.plan.clipboard {
        if let Err(error) = (outputs.copy)(&payload, outputs.osc52_fallback) {
            errors.push(error.to_string());
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; ").into())
    }
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
    if !quiet {
        let untracked = match mode {
            Mode::Default | Mode::Unstaged | Mode::All => count_untracked(cwd, git_program)?,
            _ => 0,
        };
        writeln!(messages, "{}", empty_message(mode, untracked))?;
    }
    Ok(())
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
mod tests {
    use std::{ffi::OsString, fs, path::Path, process::Command};

    use clap::Parser;

    use super::{Environment, OutputPlan, output_plan, run_with_outputs};
    use crate::{
        cli::Cli,
        config::{Config, EffectiveConfig},
    };

    fn changed_repo() -> tempfile::TempDir {
        let repo = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let status = Command::new("git")
                .args(args)
                .current_dir(repo.path())
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env(
                    "GIT_CONFIG_GLOBAL",
                    if cfg!(windows) { "NUL" } else { "/dev/null" },
                )
                .status()
                .unwrap();
            assert!(status.success());
        };
        git(&["init", "-b", "main"]);
        git(&["config", "user.name", "Test User"]);
        git(&["config", "user.email", "test@example.invalid"]);
        fs::write(repo.path().join("tracked.txt"), "before\n").unwrap();
        git(&["add", "tracked.txt"]);
        git(&["commit", "-m", "initial"]);
        fs::write(repo.path().join("tracked.txt"), "after café 😀\n").unwrap();
        repo
    }

    fn environment(repo: &Path) -> Environment {
        Environment {
            cwd: repo.to_path_buf(),
            config_path: Some(repo.join("missing-config.toml")),
            git_program: OsString::from("git"),
        }
    }

    #[test]
    fn output_plan_covers_terminal_and_non_terminal_destinations() {
        for (args, terminal, expected) in [
            (&["gd"][..], true, OutputPlan::new(false, true, false)),
            (&["gd", "-c"][..], true, OutputPlan::new(false, false, true)),
            (&["gd", "-C"][..], true, OutputPlan::new(false, true, true)),
            (&["gd"][..], false, OutputPlan::new(true, false, false)),
            (&["gd", "-c"][..], false, OutputPlan::new(true, false, true)),
            (&["gd", "-C"][..], false, OutputPlan::new(true, false, true)),
            (
                &["gd", "-p", "-C"][..],
                true,
                OutputPlan::new(true, true, true),
            ),
            (
                &["gd", "-C", "-o", "out"][..],
                false,
                OutputPlan::new(true, true, true),
            ),
            (
                &["gd", "-o", "out"][..],
                false,
                OutputPlan::new(true, true, false),
            ),
        ] {
            assert_eq!(output_plan(&Cli::parse_from(args), terminal), expected);
        }
    }

    #[test]
    fn configured_output_directory_does_not_suppress_automatic_stdout() {
        let cli = Cli::parse_from(["gd"]);
        let config = Config {
            output_dir: "configured-diffs".into(),
            quiet: false,
            base_branch: None,
            ..Config::default()
        };

        assert!(output_plan(&cli, false).stdout);
        assert_eq!(
            EffectiveConfig::new(&config, &cli, Path::new("/repo")).output_dir,
            Path::new("/repo/configured-diffs")
        );
    }

    #[test]
    fn copy_destinations_receive_the_same_rendered_payload() {
        for (args, terminal, expect_stdout, expect_file) in [
            (&["gd", "-c"][..], true, false, false),
            (&["gd", "-C"][..], true, false, true),
            (&["gd", "-C"][..], false, true, false),
            (&["gd", "-p", "-C"][..], true, true, true),
            (&["gd", "-C", "-o", "out"][..], false, true, true),
        ] {
            let repo = changed_repo();
            let cli = Cli::parse_from(args);
            let plan = output_plan(&cli, terminal);
            let mut stdout = Vec::new();
            let mut messages = Vec::new();
            let mut copied = Vec::new();
            run_with_outputs(
                cli,
                environment(repo.path()),
                plan,
                &mut stdout,
                &mut messages,
                &mut |payload, _fallback| {
                    copied.extend_from_slice(payload);
                    Ok(())
                },
            )
            .unwrap();

            let expected = Command::new("git")
                .args(["diff", "--no-color"])
                .current_dir(repo.path())
                .output()
                .unwrap()
                .stdout;
            assert_eq!(copied, expected);
            assert_eq!(
                stdout,
                if expect_stdout {
                    expected.clone()
                } else {
                    Vec::new()
                }
            );
            let output_dir = if args.contains(&"out") { "out" } else { "." };
            let saved = repo.path().join(output_dir).join("unstaged.diff");
            assert_eq!(saved.exists(), expect_file);
            if expect_file {
                assert_eq!(fs::read(saved).unwrap(), expected);
            }
        }
    }

    #[test]
    fn copy_save_keeps_a_successful_file_when_clipboard_fails() {
        let repo = changed_repo();
        let cli = Cli::parse_from(["gd", "-C"]);
        let mut copied_stdout = Vec::new();
        let error = run_with_outputs(
            cli,
            environment(repo.path()),
            OutputPlan::new(false, true, true),
            &mut copied_stdout,
            &mut Vec::new(),
            &mut |_payload, _fallback| Err("clipboard unavailable".into()),
        )
        .unwrap_err()
        .to_string();

        assert!(repo.path().join("unstaged.diff").exists());
        assert!(error.contains("clipboard unavailable"), "{error}");
    }

    #[test]
    fn file_failure_does_not_undo_a_successful_clipboard_write() {
        let repo = changed_repo();
        fs::write(repo.path().join("blocked"), "not a directory").unwrap();
        let cli = Cli::parse_from(["gd", "-C", "-o", "blocked"]);
        let mut copied = Vec::new();
        let error = run_with_outputs(
            cli,
            environment(repo.path()),
            OutputPlan::new(false, true, true),
            &mut Vec::new(),
            &mut Vec::new(),
            &mut |payload, _fallback| {
                copied.extend_from_slice(payload);
                Ok(())
            },
        )
        .unwrap_err()
        .to_string();

        assert!(!copied.is_empty());
        assert!(error.contains("cannot create output directory"), "{error}");
    }

    #[test]
    fn configured_local_osc52_fallback_reaches_clipboard_routing() {
        let repo = changed_repo();
        let config = repo.path().join("config.toml");
        fs::write(&config, "[clipboard]\nosc52_fallback = true\n").unwrap();
        let mut environment = environment(repo.path());
        environment.config_path = Some(config);
        let mut fallback = false;

        run_with_outputs(
            Cli::parse_from(["gd", "-c"]),
            environment,
            OutputPlan::new(false, false, true),
            &mut Vec::new(),
            &mut Vec::new(),
            &mut |_payload, configured| {
                fallback = configured;
                Ok(())
            },
        )
        .unwrap();

        assert!(fallback);
    }
}
