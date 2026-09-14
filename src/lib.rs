pub mod app;
pub mod cli;
pub mod config;
pub mod git;

use clap::FromArgMatches;

pub fn run(binary_name: &'static str) {
    let matches = cli::Cli::command_for(binary_name).get_matches();
    let cli = cli::Cli::from_arg_matches(&matches)
        .and_then(cli::Cli::validated)
        .unwrap_or_else(|error| error.exit());
    if let Err(error) = app::run(cli) {
        eprintln!("{binary_name}: {error}");
        std::process::exit(1);
    }
}
