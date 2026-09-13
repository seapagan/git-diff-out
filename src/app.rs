use std::{
    error::Error,
    ffi::OsString,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use tempfile::NamedTempFile;

use crate::{
    cli::{Cli, Mode},
    config::{Config, EffectiveConfig},
    git::{detect_base, resolved_diff_args},
};

pub fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    run_in(cli, Environment::system()?)
}

pub struct Environment {
    pub cwd: PathBuf,
    pub config_path: Option<PathBuf>,
    pub git_program: OsString,
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
    run_in_with_writer(cli, environment, &mut io::stdout().lock())
}

pub fn run_in_with_writer(
    cli: Cli,
    environment: Environment,
    messages: &mut dyn Write,
) -> Result<(), Box<dyn Error>> {
    let mode = cli.mode()?;
    if cli.stdout {
        match &mode {
            Mode::Branch(None) => {}
            Mode::Branch(Some(base)) => {
                return run_to_stdout(
                    &mode,
                    Some(base),
                    &environment.cwd,
                    &environment.git_program,
                );
            }
            _ => {
                return run_to_stdout(&mode, None, &environment.cwd, &environment.git_program);
            }
        }
    }

    let config = match environment.config_path {
        Some(path) => Config::load(&path)?,
        None => Config::default(),
    };
    let effective = EffectiveConfig::new(&config, &cli, &environment.cwd);
    let base = match &mode {
        Mode::Branch(Some(explicit)) => Some(explicit.clone()),
        Mode::Branch(None) => Some(match config.base_branch {
            Some(configured) => configured,
            None => detect_base(&environment.cwd, &environment.git_program)?,
        }),
        _ => None,
    };

    if cli.stdout {
        run_to_stdout(
            &mode,
            base.as_deref(),
            &environment.cwd,
            &environment.git_program,
        )?;
    } else {
        run_to_file(
            &mode,
            base.as_deref(),
            &environment.cwd,
            &environment.git_program,
            &effective.output_dir,
            effective.quiet,
            messages,
        )?;
    }
    Ok(())
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
    fs::create_dir_all(output_dir).map_err(|error| {
        format!(
            "cannot create output directory '{}': {error}",
            output_dir.display()
        )
    })?;
    let destination = output_dir.join(mode.filename());
    let temporary = NamedTempFile::new_in(output_dir).map_err(|error| {
        format!(
            "cannot create temporary patch in '{}': {error}",
            output_dir.display()
        )
    })?;
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

    let bytes = temporary.as_file().metadata()?.len();
    if bytes == 0 {
        drop(temporary);
        match fs::remove_file(&destination) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "cannot remove stale patch '{}': {error}",
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
        return Ok(());
    }

    temporary.persist(&destination).map_err(|error| {
        format!(
            "cannot replace patch '{}': {}",
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
