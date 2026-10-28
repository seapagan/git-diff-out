use super::{changed_repo, environment};
use crate::{
    app::{OutputPlan, run_with_outputs},
    cli::Cli,
};
use clap::Parser;
use std::fs;

#[test]
fn invalid_config_prevents_all_diff_execution_and_destinations() {
    let repo = changed_repo();
    let config = repo.path().join(".git/invalid.toml");
    fs::write(&config, "this is not toml").unwrap();
    for mode in [
        &[][..],
        &["u"],
        &["s"],
        &["all"],
        &["all", "-t"],
        &["b"],
        &["b", "main"],
        &["1"],
    ] {
        for flags in [&[][..], &["-N"], &["-H"], &["-q"], &["--note", "review"]] {
            for plan in [
                OutputPlan::new(true, false, false),
                OutputPlan::new(false, true, false),
                OutputPlan::new(false, false, true),
                OutputPlan::new(true, true, true),
            ] {
                let cli = Cli::parse_from(
                    std::iter::once("gd")
                        .chain(mode.iter().copied())
                        .chain(flags.iter().copied()),
                );
                let path = repo.path().join(cli.mode().unwrap().filename());
                fs::write(&path, "previous valid diff\n").unwrap();
                let mut environment = environment(repo.path());
                environment.config_path = Some(config.clone());
                environment.git_program = repo.path().join("must-not-run-git").into();
                let mut stdout = Vec::new();
                let mut messages = Vec::new();
                let error = run_with_outputs(
                    cli,
                    environment,
                    plan,
                    &mut stdout,
                    &mut messages,
                    &mut |_, _| panic!("clipboard must not be called"),
                )
                .unwrap_err()
                .to_string();
                assert!(error.contains("invalid config"), "{error}");
                assert!(stdout.is_empty() && messages.is_empty());
                assert_eq!(fs::read(path).unwrap(), b"previous valid diff\n");
            }
        }
    }
}
