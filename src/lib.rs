pub mod app;
pub mod cli;
mod clipboard;
pub mod config;
pub mod git;

use clap::FromArgMatches;
use clap_complete::aot::generate;

pub fn run(binary_name: &'static str) {
    let matches = cli::Cli::command_for(binary_name).get_matches();
    let cli = cli::Cli::from_arg_matches(&matches)
        .and_then(cli::Cli::validated)
        .unwrap_or_else(|error| error.exit());
    if let Some(cli::CliCommand::Completions { shell }) = cli.command.as_ref() {
        generate(
            *shell,
            &mut cli::Cli::command_for(binary_name),
            binary_name,
            &mut std::io::stdout(),
        );
        return;
    }
    if let Err(error) = app::run(cli) {
        eprintln!("{binary_name}: {error}");
        std::process::exit(1);
    }
}
