pub mod app;
pub mod cli;
mod clipboard;
pub mod config;
pub mod git;

use clap::FromArgMatches;
use clap_complete::Generator;
use clap_complete::aot::{Bash, Shell, generate};

pub fn run(binary_name: &'static str) {
    let matches = cli::Cli::command_for(binary_name).get_matches();
    let cli = cli::Cli::from_arg_matches(&matches)
        .and_then(cli::Cli::validated)
        .unwrap_or_else(|error| error.exit());
    if let Some(cli::CliCommand::Completions { shell }) = cli.command.as_ref() {
        if *shell == Shell::Bash && binary_name == "git-diff-out" {
            let mut command = cli::Cli::command_for(binary_name);
            // Build Bash's command paths without hyphens to avoid clap-rs/clap#6421.
            command.set_bin_name("git__diff__out");
            command.build();
            command.set_bin_name(binary_name);
            Bash.generate(&command, &mut std::io::stdout());
        } else {
            generate(
                *shell,
                &mut cli::Cli::command_for(binary_name),
                binary_name,
                &mut std::io::stdout(),
            );
        }
        return;
    }
    if let Err(error) = app::run(cli) {
        eprintln!("{binary_name}: {error}");
        std::process::exit(1);
    }
}
