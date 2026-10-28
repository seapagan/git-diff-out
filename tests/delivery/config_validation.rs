use super::common;

use clap::Parser;
use common::Repo;
use git_diff_out::{app, cli::Cli};
use std::fs;

#[test]
fn public_diff_entrypoints_validate_config_before_git_or_output() {
    let repo = Repo::new("main");
    let config = repo.path().join(".git/invalid.toml");
    fs::write(&config, "this is not toml").unwrap();
    for args in [
        &["-N", "-p"][..],
        &["u", "-p", "-q"],
        &["s", "-o", "out"],
        &["all", "-N", "-p"],
        &["all", "-t", "-N", "-p"],
        &["b", "-N", "-p"],
        &["b", "main", "-N", "-p"],
        &["1", "-N", "-p"],
        &["-c", "-N"],
        &["-C", "-p"],
        &["-C", "-o", "out", "-H"],
    ] {
        for writer in [false, true] {
            let cli = Cli::parse_from(std::iter::once("gd").chain(args.iter().copied()));
            let environment = app::Environment {
                cwd: repo.path().to_path_buf(),
                config_path: Some(config.clone()),
                git_program: repo.path().join("must-not-run-git").into(),
            };
            let mut messages = Vec::new();
            let result = if writer {
                app::run_in_with_writer(cli, environment, &mut messages)
            } else {
                app::run_in(cli, environment)
            };
            let error = result.unwrap_err().to_string();
            assert!(error.contains("invalid config"), "{args:?}: {error}");
            assert!(messages.is_empty());
            assert!(!repo.path().join("out").exists());
        }
    }
}

#[cfg(unix)]
#[test]
fn administrative_commands_remain_independent_of_invalid_configuration() {
    let repo = Repo::new("main");
    let config = if cfg!(target_os = "macos") {
        ".gd-test-home/Library/Application Support/git-diff-out/config.toml"
    } else {
        ".gd-test-home/git-diff-out/config.toml"
    };
    repo.write(config, "this is not toml");
    let diff = repo.gd(&["-N", "-p"]);
    assert_eq!(diff.status.code(), Some(1));
    assert!(diff.stdout.is_empty());
    assert!(String::from_utf8_lossy(&diff.stderr).contains("invalid config"));
    for args in [&["--help"][..], &["--version"], &["completions", "bash"]] {
        let output = repo.gd(args);
        common::assert_success(&output);
        assert!(!output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
    assert!(!repo.path().join("unstaged.diff").exists());
}
