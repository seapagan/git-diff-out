use std::{ffi::OsString, fs, path::Path, process::Command};

use clap::Parser;

use super::{Environment, OutputPlan, output_plan, run_with_outputs};
use crate::{
    cli::Cli,
    config::{Config, EffectiveConfig},
};

fn changed_repo() -> tempfile::TempDir {
    let repo = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(repo.path())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            )
            .status()
            .unwrap();
        assert!(status.success());
    };
    git(&["init", "-b", "main"]);
    git(&["config", "user.name", "Test User"]);
    git(&["config", "user.email", "test@example.invalid"]);
    fs::write(repo.path().join("tracked.txt"), "before\n").unwrap();
    git(&["add", "tracked.txt"]);
    git(&["commit", "-m", "initial"]);
    fs::write(repo.path().join("tracked.txt"), "after café 😀\n").unwrap();
    repo
}

fn environment(repo: &Path) -> Environment {
    Environment {
        cwd: repo.to_path_buf(),
        config_path: Some(repo.join("missing-config.toml")),
        git_program: OsString::from("git"),
    }
}

#[test]
fn output_plan_covers_terminal_and_non_terminal_destinations() {
    for (args, terminal, expected) in [
        (&["gd"][..], true, OutputPlan::new(false, true, false)),
        (&["gd", "-c"][..], true, OutputPlan::new(false, false, true)),
        (&["gd", "-C"][..], true, OutputPlan::new(false, true, true)),
        (&["gd"][..], false, OutputPlan::new(true, false, false)),
        (&["gd", "-c"][..], false, OutputPlan::new(true, false, true)),
        (&["gd", "-C"][..], false, OutputPlan::new(true, false, true)),
        (
            &["gd", "-p", "-C"][..],
            true,
            OutputPlan::new(true, true, true),
        ),
        (
            &["gd", "-C", "-o", "out"][..],
            false,
            OutputPlan::new(true, true, true),
        ),
        (
            &["gd", "-o", "out"][..],
            false,
            OutputPlan::new(true, true, false),
        ),
    ] {
        assert_eq!(output_plan(&Cli::parse_from(args), terminal), expected);
    }
}

#[test]
fn configured_output_directory_does_not_suppress_automatic_stdout() {
    let cli = Cli::parse_from(["gd"]);
    let config = Config {
        output_dir: "configured-diffs".into(),
        quiet: false,
        base_branch: None,
        ..Config::default()
    };

    assert!(output_plan(&cli, false).stdout);
    assert_eq!(
        EffectiveConfig::new(&config, &cli, Path::new("/repo")).output_dir,
        Path::new("/repo/configured-diffs")
    );
}

#[test]
fn copy_destinations_receive_the_same_rendered_payload() {
    for (args, terminal, expect_stdout, expect_file) in [
        (&["gd", "-c"][..], true, false, false),
        (&["gd", "-C"][..], true, false, true),
        (&["gd", "-C"][..], false, true, false),
        (&["gd", "-p", "-C"][..], true, true, true),
        (&["gd", "-C", "-o", "out"][..], false, true, true),
    ] {
        let repo = changed_repo();
        let cli = Cli::parse_from(args);
        let plan = output_plan(&cli, terminal);
        let mut stdout = Vec::new();
        let mut messages = Vec::new();
        let mut copied = Vec::new();
        run_with_outputs(
            cli,
            environment(repo.path()),
            plan,
            &mut stdout,
            &mut messages,
            &mut |payload, _fallback| {
                copied.extend_from_slice(payload);
                Ok(())
            },
        )
        .unwrap();

        let expected = Command::new("git")
            .args(["diff", "--no-color"])
            .current_dir(repo.path())
            .output()
            .unwrap()
            .stdout;
        assert_eq!(copied, expected);
        assert_eq!(
            stdout,
            if expect_stdout {
                expected.clone()
            } else {
                Vec::new()
            }
        );
        let output_dir = if args.contains(&"out") { "out" } else { "." };
        let saved = repo.path().join(output_dir).join("unstaged.diff");
        assert_eq!(saved.exists(), expect_file);
        if expect_file {
            assert_eq!(fs::read(saved).unwrap(), expected);
        }
    }
}

#[test]
fn annotated_payload_is_identical_for_every_destination() {
    let repo = changed_repo();
    let status = Command::new("git")
        .args(["remote", "add", "origin", "git@github.com:seapagan/gd.git"])
        .current_dir(repo.path())
        .status()
        .unwrap();
    assert!(status.success());
    let mut stdout = Vec::new();
    let mut copied = Vec::new();

    run_with_outputs(
        Cli::parse_from(["gd", "--header", "--note", "Review carefully"]),
        environment(repo.path()),
        OutputPlan::new(true, true, true),
        &mut stdout,
        &mut Vec::new(),
        &mut |payload, _fallback| {
            copied.extend_from_slice(payload);
            Ok(())
        },
    )
    .unwrap();

    let saved = fs::read(repo.path().join("unstaged.diff")).unwrap();
    assert_eq!(stdout, copied);
    assert_eq!(stdout, saved);
    assert!(stdout.starts_with(
        b"# contents: Git diff of unstaged changes\n# repository: seapagan/gd\n# note: Review carefully\n\n"
    ));
    assert_eq!(stdout.iter().filter(|byte| **byte == b'#').count(), 3);
}

#[test]
fn empty_copy_does_not_touch_the_clipboard_and_reports_status() {
    let repo = changed_repo();
    fs::write(repo.path().join("tracked.txt"), "before\n").unwrap();
    let mut copied = false;
    let mut messages = Vec::new();

    run_with_outputs(
        Cli::parse_from(["gd", "-c"]),
        environment(repo.path()),
        OutputPlan::new(false, false, true),
        &mut Vec::new(),
        &mut messages,
        &mut |_payload, _fallback| {
            copied = true;
            Ok(())
        },
    )
    .unwrap();

    assert!(!copied);
    assert_eq!(messages, b"No unstaged changes.\n");
    assert!(!repo.path().join("unstaged.diff").exists());
}

#[test]
fn empty_header_stdout_remains_silent() {
    let repo = changed_repo();
    fs::write(repo.path().join("tracked.txt"), "before\n").unwrap();
    let mut stdout = Vec::new();
    let mut messages = Vec::new();
    let mut copied = false;

    run_with_outputs(
        Cli::parse_from(["gd", "--header"]),
        environment(repo.path()),
        OutputPlan::new(true, false, false),
        &mut stdout,
        &mut messages,
        &mut |_payload, _fallback| {
            copied = true;
            Ok(())
        },
    )
    .unwrap();

    assert!(stdout.is_empty());
    assert!(messages.is_empty());
    assert!(!copied);
    assert!(!repo.path().join("unstaged.diff").exists());
}

#[test]
fn configured_header_empty_copy_skips_clipboard_and_reports_status() {
    let repo = changed_repo();
    fs::write(repo.path().join("tracked.txt"), "before\n").unwrap();
    let config = repo.path().join(".git/gd-config.toml");
    fs::write(&config, "[header]\nenabled = true\n").unwrap();
    let mut environment = environment(repo.path());
    environment.config_path = Some(config);
    let mut copied = false;
    let mut messages = Vec::new();

    run_with_outputs(
        Cli::parse_from(["gd", "-c"]),
        environment,
        OutputPlan::new(false, false, true),
        &mut Vec::new(),
        &mut messages,
        &mut |_payload, _fallback| {
            copied = true;
            Ok(())
        },
    )
    .unwrap();

    assert!(!copied);
    assert_eq!(messages, b"No unstaged changes.\n");
    assert!(!repo.path().join("unstaged.diff").exists());
}

#[test]
fn empty_note_copy_save_skips_clipboard_and_removes_stale_file() {
    let repo = changed_repo();
    fs::write(repo.path().join("tracked.txt"), "before\n").unwrap();
    fs::write(repo.path().join("unstaged.diff"), "stale\n").unwrap();
    let mut copied = false;
    let mut messages = Vec::new();

    run_with_outputs(
        Cli::parse_from(["gd", "-C", "--note", "Review carefully"]),
        environment(repo.path()),
        OutputPlan::new(false, true, true),
        &mut Vec::new(),
        &mut messages,
        &mut |_payload, _fallback| {
            copied = true;
            Ok(())
        },
    )
    .unwrap();

    assert!(!copied);
    assert_eq!(messages, b"No unstaged changes.\n");
    assert!(!repo.path().join("unstaged.diff").exists());
}

#[test]
fn empty_copy_save_does_not_touch_the_clipboard_and_removes_stale_file() {
    let repo = changed_repo();
    fs::write(repo.path().join("tracked.txt"), "before\n").unwrap();
    fs::write(repo.path().join("unstaged.diff"), "stale\n").unwrap();
    let mut copied = false;
    let mut messages = Vec::new();

    run_with_outputs(
        Cli::parse_from(["gd", "-C"]),
        environment(repo.path()),
        OutputPlan::new(false, true, true),
        &mut Vec::new(),
        &mut messages,
        &mut |_payload, _fallback| {
            copied = true;
            Ok(())
        },
    )
    .unwrap();

    assert!(!copied);
    assert_eq!(messages, b"No unstaged changes.\n");
    assert!(!repo.path().join("unstaged.diff").exists());
}

#[test]
fn copy_save_keeps_a_successful_file_when_clipboard_fails() {
    let repo = changed_repo();
    let cli = Cli::parse_from(["gd", "-C"]);
    let mut copied_stdout = Vec::new();
    let error = run_with_outputs(
        cli,
        environment(repo.path()),
        OutputPlan::new(false, true, true),
        &mut copied_stdout,
        &mut Vec::new(),
        &mut |_payload, _fallback| Err("clipboard unavailable".into()),
    )
    .unwrap_err()
    .to_string();

    assert!(repo.path().join("unstaged.diff").exists());
    assert!(error.contains("clipboard unavailable"), "{error}");
}

#[test]
fn file_failure_does_not_undo_a_successful_clipboard_write() {
    let repo = changed_repo();
    fs::write(repo.path().join("blocked"), "not a directory").unwrap();
    let cli = Cli::parse_from(["gd", "-C", "-o", "blocked"]);
    let mut copied = Vec::new();
    let error = run_with_outputs(
        cli,
        environment(repo.path()),
        OutputPlan::new(false, true, true),
        &mut Vec::new(),
        &mut Vec::new(),
        &mut |payload, _fallback| {
            copied.extend_from_slice(payload);
            Ok(())
        },
    )
    .unwrap_err()
    .to_string();

    assert!(!copied.is_empty());
    assert!(error.contains("cannot create output directory"), "{error}");
}

#[test]
fn configured_local_osc52_fallback_reaches_clipboard_routing() {
    let repo = changed_repo();
    let config = repo.path().join("config.toml");
    fs::write(&config, "[clipboard]\nosc52_fallback = true\n").unwrap();
    let mut environment = environment(repo.path());
    environment.config_path = Some(config);
    let mut fallback = false;

    run_with_outputs(
        Cli::parse_from(["gd", "-c"]),
        environment,
        OutputPlan::new(false, false, true),
        &mut Vec::new(),
        &mut Vec::new(),
        &mut |_payload, configured| {
            fallback = configured;
            Ok(())
        },
    )
    .unwrap();

    assert!(fallback);
}
