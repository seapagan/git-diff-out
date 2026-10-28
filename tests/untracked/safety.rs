use super::common::{Repo, assert_success, gd_command, patch};
use std::{fs, process::Stdio};

#[test]
fn custom_output_combined_with_redirected_stdout_is_repeatable() {
    let repo = Repo::new("main");
    super::use_empty_excludes(&repo);
    repo.write("source", "new content\n");
    repo.write("review patches/legitimate.diff", "include me\n");
    for _ in 0..2 {
        let path = repo.path().join("redirect.patch");
        let output = gd_command(
            repo.path(),
            &["all", "-o", "review patches", "--note", "Review"],
        )
        .stdout(Stdio::from(fs::File::create(&path).unwrap()))
        .output()
        .unwrap();
        assert_success(&output);
        let saved = patch(&repo, "review patches/uncommitted.diff");
        assert_eq!(fs::read(path).unwrap(), saved);
        let text = String::from_utf8(saved).unwrap();
        assert!(text.contains("+new content") && text.contains("+include me"));
        assert!(
            !text.contains("b/redirect.patch")
                && !text.contains("b/review patches/uncommitted.diff")
        );
        assert!(
            !text
                .lines()
                .any(|line| line.starts_with("diff --git") && line.contains(".tmp"))
        );
        assert_eq!(
            fs::read_dir(repo.path().join("review patches"))
                .unwrap()
                .count(),
            2
        );
    }
}

#[test]
fn successful_index_operations_clean_the_temporary_directory() {
    let repo = Repo::new("main");
    repo.write("new", "content\n");
    let temporary = tempfile::tempdir().unwrap();
    let output = gd_command(repo.path(), &["all"])
        .env("TMPDIR", temporary.path())
        .env("TMP", temporary.path())
        .env("TEMP", temporary.path())
        .output()
        .unwrap();
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("+content"));
    assert_eq!(fs::read_dir(temporary.path()).unwrap().count(), 0);
}

#[test]
fn tracked_destination_name_is_not_filtered() {
    let repo = Repo::new("main");
    repo.write("uncommitted.diff", "before\n");
    repo.git(["add", "-f", "uncommitted.diff"]);
    repo.git(["commit", "-m", "initial"]);
    repo.write("uncommitted.diff", "after\n");
    assert_eq!(
        repo.gd(&["all"]).stdout,
        repo.git(["diff", "--no-color", "HEAD"]).stdout
    );
}

#[test]
fn alternate_index_controls_which_ignored_additions_are_tracked() {
    let repo = Repo::new("main");
    repo.write(".gitignore", "ignored\n");
    repo.commit_all("initial");
    repo.write("ignored", "alternate staged content\n");
    let alternate = repo.path().join(".git/alternate");
    fs::copy(repo.path().join(".git/index"), &alternate).unwrap();
    let staged = std::process::Command::new("git")
        .args(["add", "-f", "ignored"])
        .current_dir(repo.path())
        .env("GIT_INDEX_FILE", &alternate)
        .output()
        .unwrap();
    assert!(staged.status.success());
    let real = fs::read(repo.path().join(".git/index")).unwrap();
    let other = fs::read(&alternate).unwrap();
    fs::create_dir(repo.path().join("subdir")).unwrap();
    let output = gd_command(&repo.path().join("subdir"), &["all"])
        .env("GIT_DIR", "../.git")
        .env("GIT_WORK_TREE", "..")
        .env("GIT_INDEX_FILE", ".git/alternate")
        .output()
        .unwrap();
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("+alternate staged content"));
    assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), real);
    assert_eq!(fs::read(alternate).unwrap(), other);
    assert!(repo.gd(&["all"]).stdout.is_empty());
}

#[cfg(unix)]
#[test]
fn git_failures_preserve_real_index_previous_output_and_clean_temporary_index() {
    use clap::Parser;
    use git_diff_out::{app, cli::Cli};
    use std::os::unix::fs::PermissionsExt;
    for operation in ["ls-files", "add", "diff", "read-tree"] {
        let repo = Repo::new("main");
        super::use_empty_excludes(&repo);
        repo.write("new", "content\n");
        if operation != "read-tree" {
            repo.git(["add", "new"]);
        }
        repo.write("another", "untracked content\n");
        repo.write("uncommitted.diff", "previous valid diff\n");
        let index = fs::read(repo.path().join(".git/index")).ok();
        let script = repo.path().join(".git/failing-git");
        let body = format!(
            r#"#!/bin/sh
if [ -n "$GIT_INDEX_FILE" ]; then
    printf '%s' "$GIT_INDEX_FILE" > .git/temporary-index-path
fi
for arg do
    if [ "$arg" = "{operation}" ]; then
        printf '%s\n' 'injected Git failure' >&2
        exit 23
    fi
done
exec git "$@"
"#
        );
        fs::write(&script, body).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let error = app::run_in_with_writer(
            Cli::parse_from(["gd", "all"]),
            app::Environment {
                cwd: repo.path().to_path_buf(),
                config_path: None,
                git_program: script.into_os_string(),
            },
            &mut Vec::new(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains(operation), "{error}");
        assert_eq!(fs::read(repo.path().join(".git/index")).ok(), index);
        assert_eq!(patch(&repo, "uncommitted.diff"), b"previous valid diff\n");
        if let Ok(path) = fs::read_to_string(repo.path().join(".git/temporary-index-path")) {
            assert!(!std::path::Path::new(&path).parent().unwrap().exists());
        }
    }
}

#[cfg(unix)]
#[test]
fn dangling_output_symlink_is_excluded_without_hiding_its_target() {
    use std::os::unix::fs::symlink;
    let repo = Repo::new("main");
    super::use_empty_excludes(&repo);
    repo.write("source", "content\n");
    symlink("missing-target", repo.path().join("uncommitted.diff")).unwrap();
    let output = repo.gd(&["all"]);
    assert_success(&output);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("b/uncommitted.diff"));
    repo.write("missing-target", "legitimate target content\n");
    symlink("uncommitted.diff", repo.path().join("legitimate-link")).unwrap();
    let output = repo.gd(&["all"]);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("+legitimate target content"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("b/legitimate-link"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("b/uncommitted.diff"));
}

#[test]
fn unavailable_temporary_index_directory_fails_without_changing_index_or_output() {
    let repo = Repo::new("main");
    repo.write("tracked", "before\n");
    repo.commit_all("initial");
    repo.write("new", "new content\n");
    repo.write("uncommitted.diff", "previous diff\n");
    let index = fs::read(repo.path().join(".git/index")).unwrap();
    let missing = repo.path().join(".git/nonexistent-temp-dir");
    let output = gd_command(repo.path(), &["all", "-o", "."])
        .env("TMPDIR", &missing)
        .env("TMP", &missing)
        .env("TEMP", &missing)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("temporary Git index"));
    assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), index);
    assert_eq!(patch(&repo, "uncommitted.diff"), b"previous diff\n");
}

#[test]
fn corrupt_head_is_an_error_instead_of_an_empty_tree_diff() {
    let repo = Repo::new("main");
    repo.write("tracked", "committed content\n");
    repo.commit_all("initial");
    repo.write("new", "new content\n");
    repo.write("uncommitted.diff", "previous diff\n");
    let index = fs::read(repo.path().join(".git/index")).unwrap();
    fs::write(repo.path().join(".git/refs/heads/main"), "INVALID\n").unwrap();
    for args in [&["all"][..], &["all", "-t"][..]] {
        let output = repo.gd(args);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("HEAD"));
        assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), index);
        assert_eq!(patch(&repo, "uncommitted.diff"), b"previous diff\n");
    }
}

#[cfg(unix)]
#[test]
fn untracked_stdout_streams_before_git_finishes() {
    use std::{io::Read, os::unix::fs::PermissionsExt, sync::mpsc, time::Duration};
    let repo = Repo::new("main");
    repo.write("source", "content\n");
    let wrapper_dir = repo.path().join(".git/wrappers");
    fs::create_dir(&wrapper_dir).unwrap();
    let wrapper = wrapper_dir.join("git");
    fs::write(
        &wrapper,
        r#"#!/bin/sh
if [ "$1" = diff ]; then
    printf 'diff prefix\n'
    while [ ! -f .git/release-diff ]; do sleep 0.01; done
    exit 0
fi
exec "$GD_TEST_REAL_GIT" "$@"
"#,
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    let inherited = std::env::var_os("PATH").unwrap();
    let real_git = std::env::split_paths(&inherited)
        .map(|path| path.join("git"))
        .find(|path| path.is_file())
        .unwrap();
    let paths = std::iter::once(wrapper_dir).chain(std::env::split_paths(&inherited));
    let mut child = gd_command(repo.path(), &["all", "-p"])
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("GD_TEST_REAL_GIT", real_git)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut prefix = [0; 12];
        let result = stdout.read_exact(&mut prefix).map(|()| prefix);
        send.send(result).unwrap();
    });
    let streamed = receive.recv_timeout(Duration::from_secs(5));
    fs::write(repo.path().join(".git/release-diff"), "release").unwrap();
    let output = child.wait_with_output().unwrap();
    reader.join().unwrap();
    assert_success(&output);
    assert_eq!(streamed.unwrap().unwrap(), *b"diff prefix\n");
}

#[test]
fn empty_all_saved_removes_stale_output_without_counting_it() {
    let repo = Repo::new("main");
    super::use_empty_excludes(&repo);
    repo.write("uncommitted.diff", "stale output\n");
    let messages = repo.gd_in(&["all"]).unwrap();
    assert_eq!(messages, b"No uncommitted changes.\n");
    assert!(!repo.path().join("uncommitted.diff").exists());
    assert!(!repo.path().join(".git/index").exists());
}

#[test]
fn redirected_stdout_excludes_untracked_hard_links_to_its_destination() {
    let repo = Repo::new("main");
    repo.write("source", "source\n");
    let expected = repo.gd(&["all", "-p"]);
    assert_success(&expected);
    let destination = repo.path().join("redirect.patch");
    fs::write(&destination, "previous output\n").unwrap();
    fs::hard_link(&destination, repo.path().join("output-hard-link")).unwrap();
    let output = gd_command(repo.path(), &["all", "-p"])
        .stdout(Stdio::from(fs::File::create(&destination).unwrap()))
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(fs::read(destination).unwrap(), expected.stdout);
}
