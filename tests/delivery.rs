mod common;
#[cfg(unix)]
#[path = "support/fake_program.rs"]
mod fake_program;

#[cfg(target_os = "linux")]
use std::env;
use std::{ffi::OsString, fs, io, process::Stdio};

use clap::Parser;
use common::{Repo, assert_success, gd_command, patch};
#[cfg(unix)]
use fake_program::executable_script;
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
        fs::read(repo.path().join("review patches λ/unstaged.diff")).unwrap(),
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
        fs::read(output_dir.path().join("unstaged.diff")).unwrap(),
        expected
    );
}

#[test]
fn output_directory_creation_error_preserves_the_blocking_file() {
    let repo = changed_repo();
    repo.write("blocked", "keep me\n");

    let error = repo.gd_in(&["--output-dir", "blocked"]).unwrap_err();

    assert!(error.contains("cannot create output directory"), "{error}");
    assert_eq!(fs::read(repo.path().join("blocked")).unwrap(), b"keep me\n");
}

#[cfg(unix)]
#[test]
fn temporary_patch_creation_error_names_the_output_directory() {
    use std::os::unix::fs::PermissionsExt;

    let repo = changed_repo();
    let output_dir = repo.path().join("read-only");
    fs::create_dir(&output_dir).unwrap();
    fs::set_permissions(&output_dir, fs::Permissions::from_mode(0o500)).unwrap();

    let error = repo.gd_in(&["--output-dir", "read-only"]).unwrap_err();

    fs::set_permissions(&output_dir, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(error.contains("cannot create temporary diff"), "{error}");
    assert!(error.contains(&output_dir.display().to_string()), "{error}");
}

#[test]
fn completed_diff_replaces_an_existing_patch() {
    let repo = changed_repo();
    repo.write("unstaged.diff", "stale\n");
    let expected = repo.git(["diff", "--no-color"]).stdout;

    repo.gd_in(&[]).unwrap();
    assert_eq!(patch(&repo, "unstaged.diff"), expected);
}

#[test]
fn successful_empty_diff_removes_stale_patch_and_reports_status() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "unchanged\n");
    repo.commit_all("initial");
    repo.write("staged.diff", "stale\n");
    repo.write("untracked.txt", "irrelevant to staged mode\n");

    let messages = repo.gd_in(&["s"]).unwrap();
    assert_eq!(messages, b"No staged changes.\n");
    assert!(!repo.path().join("staged.diff").exists());
}

#[test]
fn stale_patch_removal_error_preserves_the_destination_directory() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "unchanged\n");
    repo.commit_all("initial");
    let destination = repo.path().join("unstaged.diff");
    fs::create_dir(&destination).unwrap();

    let error = repo.gd_in(&[]).unwrap_err();

    assert!(error.contains("cannot remove stale diff"), "{error}");
    assert!(destination.is_dir());
}

#[test]
fn patch_persistence_error_preserves_the_destination_directory() {
    let repo = changed_repo();
    let destination = repo.path().join("unstaged.diff");
    fs::create_dir(&destination).unwrap();

    let error = repo.gd_in(&[]).unwrap_err();

    assert!(error.contains("cannot replace diff"), "{error}");
    assert!(destination.is_dir());
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
    assert!(!repo.path().join("unstaged.diff").exists());
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
    assert!(!repo.path().join("uncommitted.diff").exists());
}

#[test]
fn zero_untracked_files_keep_the_short_empty_message() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "unchanged\n");
    repo.commit_all("initial");

    assert_eq!(repo.gd_in(&[]).unwrap(), b"No unstaged changes.\n");
}

#[test]
fn empty_modes_report_their_specific_status() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "unchanged\n");
    repo.commit_all("initial");
    repo.git(["switch", "-c", "feature"]);
    repo.git(["commit", "--allow-empty", "-m", "empty one"]);
    repo.git(["commit", "--allow-empty", "-m", "empty two"]);

    for (args, expected) in [
        (&["staged"][..], "No staged changes.\n"),
        (&["all"][..], "No uncommitted changes.\n"),
        (&["branch", "main"][..], "No branch changes.\n"),
        (&["1"][..], "No changes in the last commit.\n"),
        (&["2"][..], "No changes in the last 2 commits.\n"),
    ] {
        assert_eq!(repo.gd_in(args).unwrap(), expected.as_bytes());
    }
}

#[test]
fn failed_git_preserves_existing_destination() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "initial\n");
    repo.commit_all("initial");
    repo.write("last-3-commits.diff", "valuable stale patch\n");

    let error = repo.gd_in(&["3"]).unwrap_err();
    assert_eq!(
        patch(&repo, "last-3-commits.diff"),
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
    assert!(!repo.path().join("unstaged.diff").exists());
}

#[test]
fn captured_stdout_selects_raw_diff_without_stdout_flag() {
    let repo = changed_repo();
    repo.commit_all("second");
    let expected = repo.git(["diff", "--no-color", "HEAD~1..HEAD"]).stdout;

    let output = repo.gd(&["1"]);
    assert_success(&output);
    assert_eq!(output.stdout, expected);
    assert!(output.stderr.is_empty());
    assert!(!repo.path().join("last-commit.diff").exists());
}

#[test]
fn configured_output_directory_does_not_suppress_captured_stdout() {
    let repo = changed_repo();
    let expected = repo.git(["diff", "--no-color"]).stdout;
    repo.write(
        ".gd-test-home/git-diff-out/config.toml",
        "output_dir = \"configured-diffs\"\n",
    );

    let output = repo.gd(&[]);
    assert_success(&output);
    assert_eq!(output.stdout, expected);
    assert!(output.stderr.is_empty());
    assert!(!repo.path().join("configured-diffs/unstaged.diff").exists());
}

#[test]
fn explicit_output_directory_is_saved_alongside_captured_stdout() {
    let repo = changed_repo();
    repo.commit_all("second");
    let expected = repo.git(["diff", "--no-color", "HEAD~1..HEAD"]).stdout;

    let output = repo.gd(&["1", "--output-dir", "review-diffs"]);
    assert_success(&output);
    assert_eq!(
        fs::read(repo.path().join("review-diffs/last-commit.diff")).unwrap(),
        expected
    );
    assert_eq!(output.stdout, expected);
    assert!(output.stderr.is_empty());
}

#[test]
fn redirected_stdout_selects_raw_diff_without_creating_patch() {
    let repo = changed_repo();
    repo.commit_all("second");
    let expected = repo.git(["diff", "--no-color", "HEAD~1..HEAD"]).stdout;
    let redirected = repo.path().join("redirected.patch");

    let output = gd_command(repo.path(), &["1"])
        .stdout(Stdio::from(fs::File::create(&redirected).unwrap()))
        .output()
        .unwrap();
    assert_success(&output);
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert_eq!(fs::read(redirected).unwrap(), expected);
    assert!(!repo.path().join("last-commit.diff").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn copy_writes_the_exact_diff_and_keeps_captured_stdout_clean() {
    let repo = changed_repo();
    repo.write("tracked.txt", "after café 😀\n");
    let expected = repo.git(["diff", "--no-color"]).stdout;
    let clipboard = repo.path().join("clipboard.bin");
    let (programs, _wl_copy) = executable_script("wl-copy");
    let path = env::join_paths(
        std::iter::once(programs.path().to_path_buf())
            .chain(env::split_paths(&env::var_os("PATH").unwrap())),
    )
    .unwrap();

    let output = gd_command(repo.path(), &["-c"])
        .env("PATH", path)
        .env("WAYLAND_DISPLAY", "wayland-test")
        .env("GD_TEST_CLIPBOARD_OUTPUT", &clipboard)
        .env_remove("SSH_CONNECTION")
        .env_remove("SSH_CLIENT")
        .env_remove("SSH_TTY")
        .output()
        .unwrap();

    assert_success(&output);
    assert_eq!(output.stdout, expected);
    assert!(output.stderr.is_empty());
    assert_eq!(fs::read(clipboard).unwrap(), expected);
    assert!(!repo.path().join("unstaged.diff").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn xdg_wayland_session_selects_wl_copy_without_wayland_display() {
    let repo = changed_repo();
    let expected = repo.git(["diff", "--no-color"]).stdout;
    let clipboard = repo.path().join("clipboard.bin");
    let (programs, _wl_copy) = executable_script("wl-copy");
    let path = env::join_paths(
        std::iter::once(programs.path().to_path_buf())
            .chain(env::split_paths(&env::var_os("PATH").unwrap())),
    )
    .unwrap();

    let output = gd_command(repo.path(), &["-c"])
        .env("PATH", path)
        .env_remove("WAYLAND_DISPLAY")
        .env("XDG_SESSION_TYPE", "WaYlAnD")
        .env("GD_TEST_CLIPBOARD_OUTPUT", &clipboard)
        .env_remove("SSH_CONNECTION")
        .env_remove("SSH_CLIENT")
        .env_remove("SSH_TTY")
        .output()
        .unwrap();

    assert_success(&output);
    assert_eq!(output.stdout, expected);
    assert!(output.stderr.is_empty());
    assert_eq!(fs::read(clipboard).unwrap(), expected);
}

#[cfg(target_os = "linux")]
#[test]
fn copy_save_with_explicit_directory_writes_all_three_destinations() {
    let repo = changed_repo();
    let expected = repo.git(["diff", "--no-color"]).stdout;
    let clipboard = repo.path().join("clipboard.bin");
    let (programs, _wl_copy) = executable_script("wl-copy");
    let path = env::join_paths(
        std::iter::once(programs.path().to_path_buf())
            .chain(env::split_paths(&env::var_os("PATH").unwrap())),
    )
    .unwrap();

    let output = gd_command(repo.path(), &["-C", "-o", "out"])
        .env("PATH", path)
        .env("WAYLAND_DISPLAY", "wayland-test")
        .env("GD_TEST_CLIPBOARD_OUTPUT", &clipboard)
        .env_remove("SSH_CONNECTION")
        .env_remove("SSH_CLIENT")
        .env_remove("SSH_TTY")
        .output()
        .unwrap();

    assert_success(&output);
    assert_eq!(output.stdout, expected);
    assert!(output.stderr.is_empty());
    assert_eq!(fs::read(clipboard).unwrap(), expected);
    assert_eq!(
        fs::read(repo.path().join("out/unstaged.diff")).unwrap(),
        expected
    );
}

#[cfg(target_os = "linux")]
#[test]
fn clipboard_failure_is_nonzero_without_contaminating_diff_stdout() {
    let repo = changed_repo();
    let expected = repo.git(["diff", "--no-color"]).stdout;
    let clipboard = repo.path().join("clipboard.bin");
    let (programs, _wl_copy) = executable_script("wl-copy");
    let path = env::join_paths(
        std::iter::once(programs.path().to_path_buf())
            .chain(env::split_paths(&env::var_os("PATH").unwrap())),
    )
    .unwrap();

    let output = gd_command(repo.path(), &["-c"])
        .env("PATH", path)
        .env("WAYLAND_DISPLAY", "wayland-test")
        .env("GD_TEST_CLIPBOARD_OUTPUT", &clipboard)
        .env("GD_TEST_CLIPBOARD_FAIL", "1")
        .env_remove("SSH_CONNECTION")
        .env_remove("SSH_CLIENT")
        .env_remove("SSH_TTY")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, expected);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("provider display is unavailable"),
        "{stderr}"
    );
    assert!(stderr.contains("wl-copy"), "{stderr}");
    assert!(!clipboard.exists());
}

#[cfg(target_os = "linux")]
#[test]
fn stdout_failure_does_not_prevent_a_clipboard_write() {
    let repo = changed_repo();
    let expected = repo.git(["diff", "--no-color"]).stdout;
    let clipboard = repo.path().join("clipboard.bin");
    let (programs, _wl_copy) = executable_script("wl-copy");
    let path = env::join_paths(
        std::iter::once(programs.path().to_path_buf())
            .chain(env::split_paths(&env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let full = fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .unwrap();

    let output = gd_command(repo.path(), &["-c"])
        .env("PATH", path)
        .env("WAYLAND_DISPLAY", "wayland-test")
        .env("GD_TEST_CLIPBOARD_OUTPUT", &clipboard)
        .env_remove("SSH_CONNECTION")
        .env_remove("SSH_CLIENT")
        .env_remove("SSH_TTY")
        .stdout(Stdio::from(full))
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot write diff to stdout"), "{stderr}");
    assert_eq!(fs::read(clipboard).unwrap(), expected);
}

#[cfg(unix)]
#[test]
fn multi_output_render_failure_does_not_create_a_destination() {
    let repo = changed_repo();
    let (_program_dir, program) = executable_script("diff-fails");

    let error = app::run_in_with_writer(
        Cli::parse_from(["gd", "-C"]),
        app::Environment {
            cwd: repo.path().to_path_buf(),
            config_path: None,
            git_program: program.into_os_string(),
        },
        &mut Vec::new(),
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("git diff failed with"), "{error}");
    assert!(!repo.path().join("unstaged.diff").exists());
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
    assert!(!repo.path().join("unstaged.diff").exists());
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
    assert!(!repo.path().join("last-2-commits.diff").exists());
}

#[test]
fn stdout_and_output_directory_conflict_at_the_cli() {
    let repo = changed_repo();
    let output = repo.gd(&["--stdout", "--output-dir", "out"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used with"));
    assert!(!repo.path().join("out/unstaged.diff").exists());
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
            String::from_utf8_lossy(&messages).starts_with("Wrote unstaged.diff ("),
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
        fs::read(repo.path().join("configured patches/unstaged.diff")).unwrap(),
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

    assert_eq!(patch(&repo, "unstaged.diff"), expected);
    assert!(String::from_utf8_lossy(&messages).starts_with("Wrote unstaged.diff ("));
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
fn unavailable_git_executable_is_actionable_for_stdout() {
    let repo = changed_repo();
    let error = app::run_in_with_writer(
        Cli::parse_from(["gd", "--stdout"]),
        app::Environment {
            cwd: repo.path().to_path_buf(),
            config_path: None,
            git_program: OsString::from("git-executable-that-does-not-exist"),
        },
        &mut Vec::new(),
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("failed to start git"), "{error}");
}

#[cfg(unix)]
#[test]
fn untracked_count_startup_failure_is_reported() {
    let repo = Repo::new("main");
    let (_program_dir, program) = executable_script("untracked-startup-fails");
    let error = app::run_in_with_writer(
        Cli::parse_from(["gd"]),
        app::Environment {
            cwd: repo.path().to_path_buf(),
            config_path: None,
            git_program: program.into_os_string(),
        },
        &mut Vec::new(),
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("failed to start git"), "{error}");
}

#[cfg(unix)]
#[test]
fn untracked_count_command_failure_is_reported() {
    let repo = Repo::new("main");
    let (_program_dir, program) = executable_script("untracked-command-fails");
    let error = app::run_in_with_writer(
        Cli::parse_from(["gd"]),
        app::Environment {
            cwd: repo.path().to_path_buf(),
            config_path: None,
            git_program: program.into_os_string(),
        },
        &mut Vec::new(),
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("git ls-files failed with"), "{error}");
}

struct FailingWriter;

impl io::Write for FailingWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("message sink failed"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn message_write_failure_does_not_discard_the_completed_patch() {
    let repo = changed_repo();

    let error = app::run_in_with_writer(
        Cli::parse_from(["gd"]),
        app::Environment {
            cwd: repo.path().to_path_buf(),
            config_path: None,
            git_program: OsString::from("git"),
        },
        &mut FailingWriter,
    )
    .unwrap_err()
    .to_string();

    assert_eq!(error, "message sink failed");
    assert!(repo.path().join("unstaged.diff").is_file());
}

#[test]
fn unborn_repository_empty_diff_succeeds() {
    let repo = Repo::new("develop");
    let messages = repo.gd_in(&[]).unwrap();
    assert!(messages.starts_with(b"No unstaged changes."));
    assert!(!repo.path().join("unstaged.diff").exists());
}

#[test]
fn forced_git_colour_never_enters_patch() {
    let repo = changed_repo();
    repo.git(["config", "color.ui", "always"]);

    repo.gd_in(&[]).unwrap();
    assert!(!patch(&repo, "unstaged.diff").contains(&0x1b));
}

#[test]
fn large_diff_is_streamed_to_a_file() {
    let repo = Repo::new("main");
    repo.write("large.txt", vec![b'a'; 2 * 1024 * 1024]);
    repo.commit_all("initial");
    repo.write("large.txt", vec![b'b'; 2 * 1024 * 1024]);
    let expected = repo.git(["diff", "--no-color"]).stdout;

    let messages = repo.gd_in(&[]).unwrap();
    assert_eq!(patch(&repo, "unstaged.diff"), expected);
    assert!(String::from_utf8_lossy(&messages).contains(" KiB)"));
}
