pub mod app;
pub mod cli;
mod clipboard;
pub mod config;
pub mod git;

use std::error::Error;
use std::io::{self, Write};

use clap::FromArgMatches;
use clap_complete::Generator;
use clap_complete::aot::{Bash, Shell};

fn generate_completions(
    binary_name: &'static str,
    shell: Shell,
    destination: &mut dyn Write,
) -> io::Result<()> {
    let mut command = cli::Cli::command_for(binary_name);
    if shell == Shell::Bash && binary_name.contains('-') {
        // Build Bash's command paths without hyphens to avoid clap-rs/clap#6421.
        command.set_bin_name(binary_name.replace('-', "__"));
        command.build();
        command.set_bin_name(binary_name);
        Bash.try_generate(&command, destination)
    } else {
        command.build();
        shell.try_generate(&command, destination)
    }
}

pub fn run(binary_name: &'static str) {
    let matches = cli::Cli::command_for(binary_name).get_matches();
    let cli = cli::Cli::from_arg_matches(&matches)
        .and_then(cli::Cli::validated)
        .unwrap_or_else(|error| error.exit());
    let result: Result<(), Box<dyn Error>> =
        if let Some(cli::CliCommand::Completions { shell }) = cli.command.as_ref() {
            generate_completions(binary_name, *shell, &mut io::stdout()).map_err(Into::into)
        } else {
            app::run(cli)
        };
    if let Err(error) = result {
        eprintln!("{binary_name}: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use clap_complete::aot::Shell;

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("completion write failed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn completion_generation_returns_write_errors() {
        let error = super::generate_completions("gd", Shell::Zsh, &mut FailingWriter)
            .expect_err("completion generation should return the write error");

        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(error.to_string(), "completion write failed");
    }
}
