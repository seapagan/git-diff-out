use std::path::PathBuf;

use clap::{Command, CommandFactory, Parser, error::ErrorKind};
use colored_text::Colorize;

fn usage(name: &str) -> String {
    format!("{name} [OPTIONS] [MODE]\n  {name} [OPTIONS] {{b|branch}} [BASE]")
}

fn after_help(name: &str) -> String {
    format!(
        r#"{}
  {name}                 Write unstaged tracked changes to unstaged.diff
  {name} s               Write staged tracked changes to staged.diff
  {name} 3               Write the last 3 commits to last-3-commits.diff
  {name} b               Write current-branch changes relative to the detected base
  {name} b develop       Write current-branch changes relative to develop
  {name} s -p            Write the staged diff to stdout

Piped or redirected stdout receives the raw diff automatically."#,
        "Examples:".bold().underline()
    )
}

#[derive(Debug, Parser)]
#[command(
    name = "gd",
    version,
    about,
    override_usage = usage("gd"),
    after_help = after_help("gd")
)]
pub struct Cli {
    /// Diff mode: u[nstaged], s[taged], a[ll], b[ranch], or a commit count.
    #[arg(value_name = "MODE")]
    mode_name: Option<String>,

    /// Base branch for branch mode.
    base: Option<String>,

    /// Force diff output to stdout.
    #[arg(short = 'p', long, conflicts_with = "output_dir")]
    pub stdout: bool,

    /// Directory in which to write the diff file.
    #[arg(short = 'o', long, value_name = "PATH", conflicts_with = "stdout")]
    pub output_dir: Option<PathBuf>,

    /// Suppress successful file-output messages.
    #[arg(short, long, conflicts_with = "verbose")]
    quiet: bool,

    /// Override quiet mode configured in the config file.
    #[arg(short = 'v', long, conflicts_with = "quiet")]
    verbose: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Mode {
    Default,
    Unstaged,
    Staged,
    All,
    Branch(Option<String>),
    Commits(usize),
}

impl Mode {
    pub fn filename(&self) -> String {
        match self {
            Self::Default | Self::Unstaged => "unstaged.diff".into(),
            Self::Staged => "staged.diff".into(),
            Self::All => "uncommitted.diff".into(),
            Self::Branch(_) => "branch.diff".into(),
            Self::Commits(1) => "last-commit.diff".into(),
            Self::Commits(count) => format!("last-{count}-commits.diff"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuietOverride {
    None,
    Quiet,
    Verbose,
}

impl Cli {
    pub fn command_for(name: &'static str) -> Command {
        Self::command()
            .name(name)
            .override_usage(usage(name))
            .after_help(after_help(name))
    }

    pub fn validated(self) -> Result<Self, clap::Error> {
        self.mode()?;
        Ok(self)
    }

    pub fn mode(&self) -> Result<Mode, clap::Error> {
        let Some(name) = self.mode_name.as_deref() else {
            return Ok(Mode::Default);
        };
        if self.base.is_some() && !matches!(name, "b" | "branch") {
            return Err(base_error());
        }

        let mode = match name {
            "u" | "unstaged" => Mode::Unstaged,
            "s" | "staged" => Mode::Staged,
            "a" | "all" => Mode::All,
            "b" | "branch" => Mode::Branch(self.base.clone()),
            value => match value.parse::<usize>() {
                Ok(count) if count > 0 => Mode::Commits(count),
                _ => return Err(value_error(value)),
            },
        };
        Ok(mode)
    }

    pub fn quiet_override(&self) -> QuietOverride {
        if self.quiet {
            QuietOverride::Quiet
        } else if self.verbose {
            QuietOverride::Verbose
        } else {
            QuietOverride::None
        }
    }
}

fn base_error() -> clap::Error {
    clap::Error::raw(
        ErrorKind::InvalidValue,
        "BASE is only valid with branch mode",
    )
}

fn value_error(value: &str) -> clap::Error {
    clap::Error::raw(
        ErrorKind::InvalidValue,
        format!("invalid mode or commit count '{value}'"),
    )
}
