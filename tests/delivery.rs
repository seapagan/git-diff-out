mod common;

use std::{ffi::OsString, fs};

use clap::Parser;
use common::{Repo, assert_success, gd_command, patch};
use git_diff_out::{app, cli::Cli};
use tempfile::tempdir;

fn changed_repo() -> Repo {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "before\n");
    repo.commit_all("initial");
    repo.write("tracked.txt", "after\n");
    repo
}

#[test]
fn relative_output_directory_with_spaces_and_unicode_is_created() {
    let repo = changed_repo();
    let expected = repo.git(["diff", "--no-color"]).stdout;
    repo.gd_in(&["-o", "review patches λ"]).unwrap();
    assert_eq!(
        fs::read(repo.path().join("review patches λ/diff.patch")).unwrap(),
        expected
    );
}

#[test]
fn absolute_output_directory_is_supported() {
    let repo = changed_repo();
    let output_dir = tempdir().unwrap();
    let expected = repo.git(["diff", "--no-color"]).stdout;
    repo.gd_in(&["-o", output_dir.path().to_str().unwrap()])
        .unwrap();
    assert_eq!(
        fs::read(output_dir.path().join("diff.patch")).unwrap(),
        expected
    );
}

#[test]
fn completed_diff_replaces_an_existing_patch() {
    let repo = changed_repo();
    repo.write("diff.patch", "stale\n");
    let expected = repo.git(["diff", "--no-color"]).stdout;

    repo.gd_in(&[]).unwrap();
    assert_eq!(patch(&repo, "diff.patch"), expected);
}

#[test]
fn successful_empty_diff_removes_stale_patch_and_reports_status() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "unchanged\n");
    repo.commit_all("initial");
    repo.write("staged.patch", "stale\n");
    repo.write("untracked.txt", "irrelevant to staged mode\n");

    let messages = repo.gd_in(&["s"]).unwrap();
    assert_eq!(messages, b"No staged changes.\n");
    assert!(!repo.path().join("staged.patch").exists());
}

#[test]
fn one_untracked_file_is_reported_but_remains_outside_the_diff() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "unchanged\n");
    repo.commit_all("initial");
    repo.write("untracked.txt", "not included\n");

    let messages = repo.gd_in(&[]).unwrap();
    assert_eq!(
        messages,
        b"No unstaged tracked changes (1 untracked file not included).\n"
    );
    assert!(!repo.path().join("diff.patch").exists());
}

#[test]
fn multiple_untracked_files_are_reported_for_all_mode() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "unchanged\n");
    repo.commit_all("initial");
    repo.write("one.txt", "one\n");
    repo.write("nested/two.txt", "two\n");
    repo.write("three.txt", "three\n");

    let messages = repo.gd_in(&["all"]).unwrap();
    assert_eq!(
        messages,
        b"No uncommitted tracked changes (3 untracked files not included).\n"
    );
    assert!(!repo.path().join("uncommitted.patch").exists());
}

#[test]
fn zero_untracked_files_keep_the_short_empty_message() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "unchanged\n");
    repo.commit_all("initial");

    assert_eq!(repo.gd_in(&[]).unwrap(), b"No unstaged changes.\n");
}

#[test]
fn failed_git_preserves_existing_destination() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "initial\n");
    repo.commit_all("initial");
    repo.write("last-3-commits.patch", "valuable stale patch\n");

    let error = repo.gd_in(&["3"]).unwrap_err();
    assert_eq!(
        patch(&repo, "last-3-commits.patch"),
        b"valuable stale patch\n"
    );
    assert!(error.contains("git diff failed"), "{error}");
}

#[test]
fn stdout_mode_emits_only_raw_patch_and_creates_no_patch() {
    let repo = changed_repo();
    let expected = repo.git(["diff", "--no-color"]).stdout;

    let output = repo.gd(&["-p"]);
    assert_success(&output);
    assert_eq!(output.stdout, expected);
    assert!(output.stderr.is_empty());
    assert!(!repo.path().join("diff.patch").exists());
}

#[test]
fn empty_stdout_mode_is_silent_and_creates_no_patch() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "unchanged\n");
    repo.commit_all("initial");
    repo.write("untracked.txt", "not included\n");

    let output = repo.gd(&["--stdout"]);
    assert_success(&output);
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert!(!repo.path().join("diff.patch").exists());
}

#[test]
fn stdout_mode_without_a_branch_default_does_not_read_config() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "unchanged\n");
    repo.commit_all("initial");
    let config_dir = tempdir().unwrap();
    let config_path = config_dir.path().join("invalid.toml");
    fs::write(&config_path, "this is not toml").unwrap();

    app::run_in_with_writer(
        Cli::parse_from(["gd", "--stdout"]),
        app::Environment {
            cwd: repo.path().to_path_buf(),
            config_path: Some(config_path),
            git_program: OsString::from("git"),
        },
        &mut Vec::new(),
    )
    .unwrap();
}

#[test]
fn normal_stdout_works_without_a_config_path() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "unchanged\n");
    repo.commit_all("initial");

    app::run_in_with_writer(
        Cli::parse_from(["gd", "--stdout"]),
        app::Environment {
            cwd: repo.path().to_path_buf(),
            config_path: None,
            git_program: OsString::from("git"),
        },
        &mut Vec::new(),
    )
    .unwrap();
}

#[test]
fn commit_and_branch_stdout_modes_match_git() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "root\n");
    repo.commit_all("root");
    repo.git(["switch", "-c", "feature"]);
    repo.write("tracked.txt", "feature one\n");
    repo.commit_all("one");
    repo.write("tracked.txt", "feature two\n");
    repo.commit_all("two");

    for (args, range) in [
        (&["2", "-p"][..], "HEAD~2..HEAD"),
        (&["b", "main", "--stdout"][..], "main...HEAD"),
    ] {
        let output = repo.gd(args);
        assert_success(&output);
        assert_eq!(
            output.stdout,
            repo.git(["diff", "--no-color", range]).stdout
        );
        assert!(output.stderr.is_empty());
    }
    assert!(!repo.path().join("last-2-commits.patch").exists());
}

#[test]
fn stdout_and_output_directory_conflict_at_the_cli() {
    let repo = changed_repo();
    let output = repo.gd(&["--stdout", "--output-dir", "out"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used with"));
    assert!(!repo.path().join("out/diff.patch").exists());
}

#[test]
fn cli_quiet_suppresses_success_messages() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "unchanged\n");
    repo.commit_all("initial");
    repo.write("untracked.txt", "not included\n");
    let messages = repo.gd_in(&["-q"]).unwrap();
    assert!(messages.is_empty());
}

#[test]
fn configured_quiet_and_verbose_precedence_control_messages() {
    let repo = changed_repo();
    let config_dir = tempdir().unwrap();
    let config_path = config_dir.path().join("config.toml");
    fs::write(&config_path, "quiet = true\n").unwrap();
    let environment = || app::Environment {
        cwd: repo.path().to_path_buf(),
        config_path: Some(config_path.clone()),
        git_program: OsString::from("git"),
    };

    let mut messages = Vec::new();
    app::run_in_with_writer(Cli::parse_from(["gd"]), environment(), &mut messages).unwrap();
    assert!(messages.is_empty());

    for flag in ["-v", "--verbose"] {
        messages.clear();
        app::run_in_with_writer(Cli::parse_from(["gd", flag]), environment(), &mut messages)
            .unwrap();
        assert!(
            String::from_utf8_lossy(&messages).starts_with("Wrote diff.patch ("),
            "configured quiet mode was not overridden by {flag}"
        );
    }
}

#[test]
fn configured_relative_output_directory_is_resolved_from_cwd() {
    let repo = changed_repo();
    let config_dir = tempdir().unwrap();
    let config_path = config_dir.path().join("config.toml");
    fs::write(
        &config_path,
        "output_dir = 'configured patches'\nquiet = true\n",
    )
    .unwrap();

    app::run_in_with_writer(
        Cli::parse_from(["gd"]),
        app::Environment {
            cwd: repo.path().to_path_buf(),
            config_path: Some(config_path),
            git_program: OsString::from("git"),
        },
        &mut Vec::new(),
    )
    .unwrap();

    assert_eq!(
        fs::read(repo.path().join("configured patches/diff.patch")).unwrap(),
        repo.git(["diff", "--no-color"]).stdout
    );
}

#[test]
fn file_output_without_a_config_path_uses_defaults() {
    let repo = changed_repo();
    let expected = repo.git(["diff", "--no-color"]).stdout;
    let mut messages = Vec::new();

    app::run_in_with_writer(
        Cli::parse_from(["gd"]),
        app::Environment {
            cwd: repo.path().to_path_buf(),
            config_path: None,
            git_program: OsString::from("git"),
        },
        &mut messages,
    )
    .unwrap();

    assert_eq!(patch(&repo, "diff.patch"), expected);
    assert!(String::from_utf8_lossy(&messages).starts_with("Wrote diff.patch ("));
}

#[test]
fn errors_remain_visible_with_quiet_mode() {
    let outside = tempdir().unwrap();
    let output = gd_command(outside.path(), &["--quiet", "--stdout"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .to_ascii_lowercase()
            .contains("not a git repository")
    );
}

#[test]
fn unavailable_git_executable_is_actionable() {
    let repo = changed_repo();
    let config_dir = tempdir().unwrap();
    let error = app::run_in_with_writer(
        Cli::parse_from(["gd", "--quiet"]),
        app::Environment {
            cwd: repo.path().to_path_buf(),
            config_path: Some(config_dir.path().join("missing.toml")),
            git_program: OsString::from("git-executable-that-does-not-exist"),
        },
        &mut Vec::new(),
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("failed to start git"), "{error}");
}

#[test]
fn unborn_repository_empty_diff_succeeds() {
    let repo = Repo::new("develop");
    let messages = repo.gd_in(&[]).unwrap();
    assert!(messages.starts_with(b"No unstaged changes."));
    assert!(!repo.path().join("diff.patch").exists());
}

#[test]
fn forced_git_colour_never_enters_patch() {
    let repo = changed_repo();
    repo.git(["config", "color.ui", "always"]);

    repo.gd_in(&[]).unwrap();
    assert!(!patch(&repo, "diff.patch").contains(&0x1b));
}

#[test]
fn large_diff_is_streamed_to_a_file() {
    let repo = Repo::new("main");
    repo.write("large.txt", vec![b'a'; 2 * 1024 * 1024]);
    repo.commit_all("initial");
    repo.write("large.txt", vec![b'b'; 2 * 1024 * 1024]);
    let expected = repo.git(["diff", "--no-color"]).stdout;

    repo.gd_in(&["--quiet"]).unwrap();
    assert_eq!(patch(&repo, "diff.patch"), expected);
}
