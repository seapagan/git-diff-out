mod common;

use std::{
    fs,
    process::{Command, Stdio},
};

use common::{Repo, assert_success, null_device, patch};
use tempfile::tempdir;

#[test]
fn default_and_unstaged_aliases_match_git_diff() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "before\n");
    repo.commit_all("initial");
    repo.write("tracked.txt", "after\n");
    let expected = repo.git(["diff", "--no-color"]).stdout;

    for (args, filename) in [
        (&[][..], "unstaged.diff"),
        (&["u"][..], "unstaged.diff"),
        (&["unstaged"][..], "unstaged.diff"),
    ] {
        repo.gd_in(args).unwrap();
        assert_eq!(patch(&repo, filename), expected);
    }
}

#[test]
fn staged_aliases_match_git_diff_staged() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "before\n");
    repo.commit_all("initial");
    repo.write("tracked.txt", "staged\n");
    repo.git(["add", "tracked.txt"]);
    let expected = repo.git(["diff", "--no-color", "--staged"]).stdout;

    for alias in ["s", "staged"] {
        repo.gd_in(&[alias]).unwrap();
        assert_eq!(patch(&repo, "staged.diff"), expected);
    }
}

#[test]
fn all_aliases_include_staged_and_unstaged_changes() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "one\ntwo\nthree\n");
    repo.commit_all("initial");
    repo.write("tracked.txt", "ONE\ntwo\nthree\n");
    repo.git(["add", "tracked.txt"]);
    repo.write("tracked.txt", "ONE\ntwo\nTHREE\n");
    let expected = repo.git(["diff", "--no-color", "HEAD"]).stdout;

    for alias in ["a", "all"] {
        repo.gd_in(&[alias]).unwrap();
        assert_eq!(patch(&repo, "uncommitted.diff"), expected);
    }
}

#[test]
fn commit_counts_match_exact_git_ranges() {
    let repo = Repo::new("main");
    repo.write("history.txt", "zero\n");
    repo.commit_all("initial");
    repo.write("history.txt", "one\n");
    repo.commit_all("one");
    repo.write("history.txt", "two\n");
    repo.commit_all("two");

    for (count, filename) in [("1", "last-commit.diff"), ("2", "last-2-commits.diff")] {
        let expected = repo
            .git(["diff", "--no-color", &format!("HEAD~{count}..HEAD")])
            .stdout;
        repo.gd_in(&[count]).unwrap();
        assert_eq!(patch(&repo, filename), expected);
    }
    let single_count = 1;
    let legacy_patch = repo.path().join(format!("last-{single_count}-commit.diff"));
    assert!(!legacy_patch.exists());
}

#[test]
fn staged_added_deleted_renamed_and_binary_files_match_git() {
    let repo = Repo::new("main");
    repo.write("delete.txt", "delete me\n");
    repo.write("old-name.txt", "rename me\n");
    repo.write("binary.bin", [0_u8, 1, 2, 3]);
    repo.commit_all("initial");

    fs::remove_file(repo.path().join("delete.txt")).unwrap();
    fs::rename(
        repo.path().join("old-name.txt"),
        repo.path().join("renamed-λ.txt"),
    )
    .unwrap();
    repo.write("added-λ.txt", "added\n");
    repo.write("binary.bin", [0_u8, 255, 2, 3, 4]);
    repo.git(["add", "-A"]);

    let expected = repo.git(["diff", "--no-color", "--staged"]).stdout;
    repo.gd_in(&["s"]).unwrap();
    assert_eq!(patch(&repo, "staged.diff"), expected);
}

#[test]
fn installed_binary_runs_a_core_stdout_mode() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "before\n");
    repo.commit_all("initial");
    repo.write("tracked.txt", "after\n");

    let output = repo.gd(&["--stdout"]);
    assert_success(&output);
    assert_eq!(output.stdout, repo.git(["diff", "--no-color"]).stdout);
}

#[test]
fn alias_stdout_matches_gd() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "before\n");
    repo.commit_all("initial");
    repo.write("tracked.txt", "after\n");

    let gd = repo.gd(&["--stdout"]);
    let isolated = repo.path().join(".gd-test-home");
    let alias = Command::new(env!("CARGO_BIN_EXE_git-diff-out"))
        .arg("--stdout")
        .current_dir(repo.path())
        .env("HOME", &isolated)
        .env("USERPROFILE", &isolated)
        .env("XDG_CONFIG_HOME", &isolated)
        .env("APPDATA", &isolated)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device())
        .output()
        .expect("git-diff-out should run");

    assert_success(&gd);
    assert_success(&alias);
    assert_eq!(alias.stdout, gd.stdout);
}

fn assert_unborn_all_uses_repository_empty_tree(repo: &Repo) {
    repo.write("tracked.txt", "staged\n");
    repo.git(["add", "tracked.txt"]);
    repo.write("tracked.txt", "staged then unstaged\n");
    repo.write("untracked.txt", "excluded\n");

    let empty_tree = Command::new("git")
        .args(["hash-object", "-t", "tree", "--stdin"])
        .current_dir(repo.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(empty_tree.status.success());
    let empty_tree = String::from_utf8(empty_tree.stdout).unwrap();
    let expected = repo.git(["diff", "--no-color", empty_tree.trim()]).stdout;

    for alias in ["a", "all"] {
        repo.gd_in(&[alias]).unwrap();
        assert_eq!(patch(repo, "uncommitted.diff"), expected);
    }
    assert_eq!(repo.git(["show", ":tracked.txt"]).stdout, b"staged\n");
    let patch = String::from_utf8(expected).unwrap();
    assert!(patch.contains("staged then unstaged"));
    assert!(!patch.contains("untracked.txt"));
}

#[test]
fn all_aliases_diff_against_the_empty_tree_with_unborn_head() {
    assert_unborn_all_uses_repository_empty_tree(&Repo::new("main"));
}

#[test]
fn unborn_all_supports_sha256_repositories_when_git_does() {
    let root = tempdir().unwrap();
    let initialized = Command::new("git")
        .args(["init", "--object-format=sha256", "-b", "main"])
        .current_dir(root.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device())
        .output()
        .unwrap();
    if !initialized.status.success() {
        return;
    }
    let repo = Repo { root };
    repo.git(["config", "user.name", "Test User"]);
    repo.git(["config", "user.email", "test@example.invalid"]);
    assert_unborn_all_uses_repository_empty_tree(&repo);
}
