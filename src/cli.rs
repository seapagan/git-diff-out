use std::path::PathBuf;

use clap::{Parser, error::ErrorKind};
use colored_text::Colorize;

const USAGE: &str = r#"gd [OPTIONS] [MODE]
  gd [OPTIONS] {b|branch} [BASE]"#;

const EXAMPLES: &str = r#"  gd                 Write unstaged tracked changes to diff.patch
  gd s               Write staged tracked changes to staged.patch
  gd 3               Write the last 3 commits to last-3-commits.patch
  gd b               Write current-branch changes relative to the detected base
  gd b develop       Write current-branch changes relative to develop
  gd s -p            Write the staged diff to stdout"#;

fn after_help() -> String {
    format!("{}\n{EXAMPLES}", "Examples:".bold().underline())
}

#[derive(Debug, Parser)]
#[command(
    name = "gd",
    version,
    about,
    override_usage = USAGE,
    after_help = after_help()
)]
pub struct Cli {
    /// Diff mode: u[nstaged], s[taged], a[ll], b[ranch], or a commit count.
    #[arg(value_name = "MODE")]
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

#[cfg(test)]
mod tests {
    use std::{env, process::Command};

    use clap::CommandFactory;
    use colored_text::{ColorMode, ColorizeConfig};

    use super::Cli;

    #[test]
    fn examples_heading_is_bold_and_underlined_when_styling_is_forced() {
        if env::var_os("NO_COLOR").is_some() {
            let output = Command::new(env::current_exe().expect("test executable should exist"))
                .args([
                    "--exact",
                    "cli::tests::examples_heading_is_bold_and_underlined_when_styling_is_forced",
                ])
                .env_remove("NO_COLOR")
                .output()
                .expect("style test should rerun without NO_COLOR");
            assert!(output.status.success());
            return;
        }

        let previous_mode = ColorizeConfig::color_mode();
        ColorizeConfig::set_color_mode(ColorMode::Always);
        let help = Cli::command().render_help().ansi().to_string();
        ColorizeConfig::set_color_mode(previous_mode);

        assert!(help.contains("\u{1b}[1;4mExamples:\u{1b}[0m"));
    }
}
