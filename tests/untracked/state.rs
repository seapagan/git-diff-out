use common::{Repo, assert_success, gd_command, null_device};
use std::{fs, process::Command};

use super::common;

fn mixed_repo(committed: bool) -> Repo {
    let repo = Repo::new("main");
    repo.write("tracked", "base\n");
    repo.git(["add", "tracked"]);
    if committed {
        repo.git(["commit", "-m", "initial"]);
    }
    repo.write("tracked", "staged\n");
    repo.git(["add", "tracked"]);
    repo.write("tracked", "final\n");
    repo.write("new/nested/untracked", "new content\n");
    repo
}

#[test]
fn mixed_changes_preserve_partial_staging_and_index_bytes() {
    for committed in [false, true] {
        let repo = mixed_repo(committed);
        let index = fs::read(repo.path().join(".git/index")).unwrap();
        let entries = repo.git(["ls-files", "--stage", "-z"]).stdout;
        let output = repo.gd(&["all", "-p"]);
        assert_success(&output);
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("+final") && text.contains("+new content"));
        assert!(!text.contains("+staged"));
        assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), index);
        assert_eq!(repo.git(["ls-files", "--stage", "-z"]).stdout, entries);
        assert_eq!(repo.git(["show", ":tracked"]).stdout, b"staged\n");
    }
}

#[test]
fn ignore_rules_cover_repository_info_and_global_exclusions() {
    let repo = Repo::new("main");
    repo.write(".gitignore", "ignored/\n*.secret\n");
    repo.write(".git/info/exclude", "info-excluded\n");
    repo.write(".git/global-ignore", "global-excluded\n");
    repo.git(["config", "core.excludesFile", ".git/global-ignore"]);
    for file in [
        "ignored/nested/file",
        "hidden.secret",
        "info-excluded",
        "global-excluded",
        "visible/deep/file",
    ] {
        repo.write(file, "content\n");
    }
    let output = repo.gd(&["all"]);
    assert_success(&output);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("visible/deep/file"));
    for file in [
        "ignored/nested/file",
        "hidden.secret",
        "info-excluded",
        "global-excluded",
    ] {
        assert!(!text.contains(&format!("b/{file}")));
    }
}

#[test]
fn staged_deletions_new_files_and_renames_match_native_git() {
    let repo = Repo::new("main");
    repo.write("deleted", "delete me\n");
    repo.write("old", "rename me\n");
    repo.commit_all("initial");
    repo.git(["rm", "deleted"]);
    fs::rename(repo.path().join("old"), repo.path().join("renamed")).unwrap();
    repo.write("staged-new", "staged content\n");
    repo.git(["add", "staged-new"]);
    repo.write("untracked", "new content\n");
    let actual = repo.gd(&["all", "-p"]);
    assert_success(&actual);
    repo.git(["add", "-N", "renamed", "untracked"]);
    assert_eq!(
        actual.stdout,
        repo.git(["diff", "--no-color", "HEAD"]).stdout
    );
}

#[test]
fn binary_and_literal_pathspec_filenames_match_native_git() {
    let repo = Repo::new("main");
    let names = vec!["space name", "-dash", "λ.txt"];
    #[cfg(unix)]
    let names = names
        .into_iter()
        .chain([":(glob)star*", "quote\"name", "line\nbreak"]);
    for name in names {
        repo.write(name, "content\n");
    }
    repo.write("binary", [0, 255, 1]);
    let output = repo.gd(&["all", "-p"]);
    assert_success(&output);
    repo.git(["add", "-N", "--all"]);
    let tree = repo.git(["hash-object", "-t", "tree", "--stdin"]);
    let tree = String::from_utf8(tree.stdout).unwrap();
    assert_eq!(
        output.stdout,
        repo.git(["diff", "--no-color", tree.trim()]).stdout
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Binary files"));
}

#[cfg(unix)]
#[test]
fn untracked_symlinks_are_preserved() {
    use std::os::unix::fs::symlink;
    let repo = Repo::new("main");
    symlink("target", repo.path().join("link")).unwrap();
    let output = repo.gd(&["all"]);
    assert_success(&output);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("120000") && text.contains("+target"));
}

#[cfg(target_os = "linux")]
#[test]
fn non_utf8_untracked_filenames_are_preserved() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let repo = Repo::new("main");
    fs::write(
        repo.path().join(OsString::from_vec(b"name-\xff".to_vec())),
        b"content\n",
    )
    .unwrap();
    let output = repo.gd(&["all"]);
    assert_success(&output);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("name-\\377") && text.contains("+content"));
}

#[test]
fn sha256_unborn_untracked_patches_apply_when_supported() {
    let root = tempfile::tempdir().unwrap();
    let initialized = Command::new("git")
        .args(["init", "--object-format=sha256", "-b", "main"])
        .current_dir(root.path())
        .env("GIT_CONFIG_GLOBAL", null_device())
        .output()
        .unwrap();
    if !initialized.status.success() {
        return;
    }
    let repo = Repo { root };
    repo.write("new", "sha256 content\n");
    let output = repo.gd(&["all"]);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("+sha256 content"));
    assert_reverse_applies(&repo, &output.stdout);
    assert!(!repo.path().join(".git/index").exists());
}

#[test]
fn split_and_alternate_indexes_are_preserved() {
    let repo = mixed_repo(true);
    repo.git(["config", "core.splitIndex", "true"]);
    repo.git(["update-index", "--split-index"]);
    let shared_indexes = || {
        let mut files: Vec<_> = fs::read_dir(repo.path().join(".git"))
            .unwrap()
            .map(Result::unwrap)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("sharedindex.")
            })
            .map(|path| {
                (
                    path.file_name().unwrap().to_os_string(),
                    fs::read(path).unwrap(),
                )
            })
            .collect();
        files.sort();
        files
    };
    let shared = shared_indexes();
    let original = fs::read(repo.path().join(".git/index")).unwrap();
    let output = repo.gd(&["all"]);
    assert_success(&output);
    assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), original);
    assert_eq!(shared_indexes(), shared);
    let alternate = repo.path().join(".git/alternate-index");
    fs::copy(repo.path().join(".git/index"), &alternate).unwrap();
    let output = gd_command(repo.path(), &["all"])
        .env("GIT_INDEX_FILE", &alternate)
        .output()
        .unwrap();
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("+new content"));
    assert_eq!(fs::read(alternate).unwrap(), original);
    assert_eq!(shared_indexes(), shared);
    assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), original);
}

#[test]
fn linked_worktrees_and_subdirectory_invocations_include_root_paths() {
    let repo = mixed_repo(true);
    let worktree = tempfile::tempdir().unwrap();
    let path = worktree.path().join("linked");
    repo.git(["worktree", "add", "-b", "linked", path.to_str().unwrap()]);
    fs::create_dir(path.join("subdir")).unwrap();
    fs::write(path.join("new"), "linked content\n").unwrap();
    let output = gd_command(&path.join("subdir"), &["all"]).output().unwrap();
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("+linked content"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("+final"));
}

#[test]
fn clean_and_tracked_only_repositories_keep_native_output() {
    let repo = Repo::new("main");
    assert_success(&repo.gd(&["all"]));
    assert!(repo.gd(&["all"]).stdout.is_empty());
    repo.write("tracked", "before\n");
    repo.commit_all("initial");
    assert!(repo.gd(&["all"]).stdout.is_empty());
    repo.write("tracked", "after\n");
    assert_eq!(
        repo.gd(&["all"]).stdout,
        repo.git(["diff", "--no-color", "HEAD"]).stdout
    );
}

fn assert_reverse_applies(repo: &Repo, patch: &[u8]) {
    use std::{io::Write, process::Stdio};
    let mut child = Command::new("git")
        .args(["apply", "--reverse", "--check"])
        .current_dir(repo.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(patch).unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn empty_untracked_files_produce_applicable_new_file_patches() {
    let repo = Repo::new("main");
    repo.write("empty", "");
    let output = repo.gd(&["all"]);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("new file mode"));
    assert_reverse_applies(&repo, &output.stdout);
}

#[cfg(unix)]
#[test]
fn untracked_executable_retains_its_mode() {
    use std::os::unix::fs::PermissionsExt;
    let repo = Repo::new("main");
    repo.write("executable", "#!/bin/sh\necho hello\n");
    fs::set_permissions(
        repo.path().join("executable"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let output = repo.gd(&["all"]);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("new file mode 100755"));
    assert_reverse_applies(&repo, &output.stdout);
}

#[test]
fn copied_index_preserves_racy_worktree_detection_and_patch_hashes() {
    use std::{
        fs::FileTimes,
        time::{Duration, UNIX_EPOCH},
    };
    for untracked in [false, true] {
        let repo = Repo::new("main");
        repo.git(["config", "core.trustctime", "false"]);
        repo.git(["config", "core.abbrev", "40"]);
        repo.write("tracked", "before!\n");
        repo.commit_all("initial");
        repo.write("tracked", "staged!\n");
        let time = UNIX_EPOCH + Duration::from_secs(1_600_000_000);
        let set_time = |path| {
            fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_times(FileTimes::new().set_modified(time))
                .unwrap()
        };
        set_time(repo.path().join("tracked"));
        repo.git(["add", "tracked"]);
        repo.write("tracked", "final!!\n");
        set_time(repo.path().join("tracked"));
        let index_path = repo.path().join(".git/index");
        set_time(index_path.clone());
        if untracked {
            repo.write("new", "new content\n");
        }
        let index = fs::read(&index_path).unwrap();
        let hash = String::from_utf8(repo.git(["hash-object", "tracked"]).stdout).unwrap();
        let output = repo.gd(&["all"]);
        assert_success(&output);
        assert!(
            String::from_utf8_lossy(&output.stdout).contains(hash.trim()),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert_eq!(fs::read(&index_path).unwrap(), index);
        assert_eq!(fs::metadata(index_path).unwrap().modified().unwrap(), time);
    }
}
