use std::path::PathBuf;

use clap::{Parser, error::ErrorKind};

#[derive(Debug, Parser)]
#[command(name = "gd", version, about)]
pub struct Cli {
    /// Diff mode: u[nstaged], s[taged], a[ll], b[ranch], or a commit count.
    mode_name: Option<String>,

    /// Base branch for branch mode.
    base: Option<String>,

    /// Write the raw patch to stdout.
    #[arg(short = 'p', long, conflicts_with = "output_dir")]
    pub stdout: bool,

    /// Directory in which to write the patch file.
    #[arg(short = 'o', long, value_name = "PATH", conflicts_with = "stdout")]
    pub output_dir: Option<PathBuf>,

    /// Suppress successful file-output messages.
    #[arg(short, long, conflicts_with = "verbose")]
    quiet: bool,

    /// Override quiet mode configured in the config file.
    #[arg(long, conflicts_with = "quiet")]
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
            Self::Default => "diff.patch".into(),
            Self::Unstaged => "unstaged.patch".into(),
            Self::Staged => "staged.patch".into(),
            Self::All => "uncommitted.patch".into(),
            Self::Branch(_) => "branch-diff.patch".into(),
            Self::Commits(1) => "last-1-commit.patch".into(),
            Self::Commits(count) => format!("last-{count}-commits.patch"),
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
    pub fn validated(self) -> Result<Self, clap::Error> {
        self.mode()?;
        Ok(self)
    }

    pub fn mode(&self) -> Result<Mode, clap::Error> {
        let Some(name) = self.mode_name.as_deref() else {
            return Ok(Mode::Default);
        };

        let mode = match name {
            "u" | "unstaged" => Mode::Unstaged,
            "s" | "staged" => Mode::Staged,
            "a" | "all" => Mode::All,
            "b" | "branch" => Mode::Branch(self.base.clone()),
            value if self.base.is_none() => match value.parse::<usize>() {
                Ok(count) if count > 0 => Mode::Commits(count),
                _ => return Err(value_error(value)),
            },
            _ => return Err(value_error(name)),
        };

        if self.base.is_some() && !matches!(mode, Mode::Branch(_)) {
            return Err(value_error(name));
        }
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

fn value_error(value: &str) -> clap::Error {
    clap::Error::raw(
        ErrorKind::InvalidValue,
        format!("invalid mode or commit count '{value}'"),
    )
}
