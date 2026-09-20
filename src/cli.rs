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
  {name} -c              Copy the unstaged diff to the clipboard
  {name} -C              Copy and save the unstaged diff

gd must be run inside a checked-out Git repository; bare repositories are not supported.

Piped or redirected stdout receives the rendered diff automatically.
Use --no-header when a downstream tool requires a raw Git diff."#,
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

    /// Copy the diff to the clipboard without saving a diff file.
    #[arg(short = 'c', long, conflicts_with_all = ["copy_save", "output_dir"])]
    pub copy: bool,

    /// Copy the diff to the clipboard and save the diff file.
    #[arg(short = 'C', long, conflicts_with = "copy")]
    pub copy_save: bool,

    /// Directory in which to write the diff file.
    #[arg(short = 'o', long, value_name = "PATH", conflicts_with = "stdout")]
    pub output_dir: Option<PathBuf>,

    /// Include an annotation header in the rendered diff.
    #[arg(short = 'H', long, conflicts_with = "no_header")]
    pub header: bool,

    /// Omit the annotation header, overriding configuration.
    #[arg(short = 'N', long, conflicts_with_all = ["header", "note"])]
    pub no_header: bool,

    /// Include an annotation header with note text; conflicts with --no-header.
    #[arg(long, value_name = "TEXT", conflicts_with = "no_header")]
    pub note: Option<String>,

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

    pub(crate) fn contents(&self, base: Option<&str>) -> String {
        match self {
            Self::Default | Self::Unstaged => "Git diff of unstaged changes".into(),
            Self::Staged => "Git diff of staged changes".into(),
            Self::All => "Git diff of all tracked changes".into(),
            Self::Branch(_) => format!(
                "Git diff of the current branch against {}",
                base.expect("branch mode needs a base")
            ),
            Self::Commits(1) => "Git diff of the last commit".into(),
            Self::Commits(count) => format!("Git diff of the last {count} commits"),
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
