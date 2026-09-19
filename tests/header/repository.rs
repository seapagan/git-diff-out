use super::{add_origin, assert_stdout, changed_repo, expected_header};

#[cfg(target_os = "linux")]
use super::common;

#[cfg(target_os = "linux")]
use std::{ffi::OsString, fs, os::unix::ffi::OsStringExt, process::Command};

#[test]
fn repository_path_supports_https_and_nested_namespaces() {
    let repo = changed_repo();
    add_origin(
        &repo,
        "https://user:secret@gitlab.example.com/team/platform/gd.git",
    );
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header(
        "Git diff of unstaged changes",
        "team/platform/gd",
        None,
        &diff,
    );

    assert_stdout(&repo, &["--header"], &expected);
}

#[test]
fn repository_line_breaks_are_visibly_escaped() {
    for (url, repository) in [
        (
            "git@example.com:owner/repo\ninjected.git",
            "owner/repo\\ninjected",
        ),
        (
            "git@example.com:owner/repo\rinjected.git",
            "owner/repo\\rinjected",
        ),
    ] {
        let repo = changed_repo();
        add_origin(&repo, url);
        let diff = repo.git(["diff", "--no-color"]).stdout;
        let expected = expected_header("Git diff of unstaged changes", repository, None, &diff);

        assert_stdout(&repo, &["--header"], &expected);
    }
}

#[test]
fn current_branch_upstream_remote_precedes_origin() {
    let repo = changed_repo();
    add_origin(&repo, "https://example.com/fallback/origin.git");
    repo.git([
        "remote",
        "add",
        "upstream",
        "ssh://git@git.example.com/team/upstream.git",
    ]);
    let head = String::from_utf8(repo.git(["rev-parse", "HEAD"]).stdout).unwrap();
    repo.git(["update-ref", "refs/remotes/upstream/main", head.trim()]);
    repo.git(["branch", "--set-upstream-to", "upstream/main"]);
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header("Git diff of unstaged changes", "team/upstream", None, &diff);

    assert_stdout(&repo, &["--header"], &expected);
}

#[test]
fn origin_precedes_other_configured_remotes() {
    let repo = changed_repo();
    repo.git([
        "remote",
        "add",
        "company",
        "https://example.com/team/company.git",
    ]);
    add_origin(&repo, "https://example.com/team/origin.git");
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header("Git diff of unstaged changes", "team/origin", None, &diff);

    assert_stdout(&repo, &["--header"], &expected);
}

#[test]
fn another_usable_remote_is_used_without_origin() {
    let repo = changed_repo();
    repo.git(["remote", "add", "local", repo.path().to_str().unwrap()]);
    repo.git([
        "remote",
        "add",
        "company",
        "git@example.com:group/project.git",
    ]);
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header("Git diff of unstaged changes", "group/project", None, &diff);

    assert_stdout(&repo, &["--header"], &expected);
}

#[test]
fn repository_root_basename_is_used_without_a_usable_remote() {
    let repo = changed_repo();
    repo.git(["remote", "add", "local", repo.path().to_str().unwrap()]);
    let repository = repo.path().file_name().unwrap().to_str().unwrap();
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header("Git diff of unstaged changes", repository, None, &diff);

    assert_stdout(&repo, &["--header"], &expected);
}

#[cfg(target_os = "linux")]
#[test]
fn non_utf8_repository_root_basename_is_rendered_lossily() {
    let parent = tempfile::tempdir().unwrap();
    let basename = OsString::from_vec(b"repository-\x80".to_vec());
    let path = parent.path().join(&basename);
    fs::create_dir(&path).unwrap();

    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(&path)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", common::null_device())
            .output()
            .unwrap();
        common::assert_success(&output);
        output
    };
    git(&["init", "-b", "main"]);
    git(&["config", "user.name", "Test User"]);
    git(&["config", "user.email", "test@example.invalid"]);
    fs::write(path.join("tracked.txt"), "before\n").unwrap();
    git(&["add", "tracked.txt"]);
    git(&["commit", "-m", "initial"]);
    fs::write(path.join("tracked.txt"), "after\n").unwrap();

    let diff = git(&["diff", "--no-color"]).stdout;
    let repository = basename.to_string_lossy();
    let expected = expected_header("Git diff of unstaged changes", &repository, None, &diff);
    let output = common::gd_command(&path, &["--header"]).output().unwrap();

    common::assert_success(&output);
    assert_eq!(output.stdout, expected);
    assert!(output.stderr.is_empty());
}

#[test]
fn detached_head_uses_origin_repository_name() {
    let repo = changed_repo();
    add_origin(&repo, "git@github.com:owner/project.git");
    repo.git(["switch", "--detach"]);
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header("Git diff of unstaged changes", "owner/project", None, &diff);

    assert_stdout(&repo, &["--header"], &expected);
}

#[test]
fn duplicate_unusable_origin_falls_back_to_repository_root() {
    let repo = changed_repo();
    add_origin(&repo, repo.path().to_str().unwrap());
    let repository = repo.path().file_name().unwrap().to_str().unwrap();
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header("Git diff of unstaged changes", repository, None, &diff);

    assert_stdout(&repo, &["--header"], &expected);
}

#[test]
fn file_remote_falls_back_to_repository_root() {
    let repo = changed_repo();
    add_origin(&repo, "file:///tmp/project.git");
    let repository = repo.path().file_name().unwrap().to_str().unwrap();
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header("Git diff of unstaged changes", repository, None, &diff);

    assert_stdout(&repo, &["--header"], &expected);
}

#[test]
fn posix_path_with_colon_falls_back_to_repository_root() {
    let repo = changed_repo();
    add_origin(&repo, "/tmp/group:repo");
    let repository = repo.path().file_name().unwrap().to_str().unwrap();
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header("Git diff of unstaged changes", repository, None, &diff);

    assert_stdout(&repo, &["--header"], &expected);
}
