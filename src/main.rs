use clap::Parser;
use git_diff_out::{app, cli::Cli};

fn main() {
    let cli = Cli::parse()
        .validated()
        .unwrap_or_else(|error| error.exit());
    if let Err(error) = app::run(cli) {
        eprintln!("gd: {error}");
        std::process::exit(1);
    }
}
