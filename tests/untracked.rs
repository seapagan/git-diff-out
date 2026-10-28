mod common;
#[path = "untracked/configuration.rs"]
mod configuration;
#[path = "untracked/safety.rs"]
mod safety;
#[path = "untracked/state.rs"]
mod state;

use common::{Repo, assert_success, gd_command, patch};
use std::{fs, process::Stdio};

#[test]
fn unborn_untracked_files_are_new_file_patches_without_creating_an_index() {
    let repo = Repo::new("main");
    repo.write("src/new file.txt", "hello untracked\n");
    let output = repo.gd(&["all", "-p"]);
    assert_success(&output);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("new file mode"));
    assert!(text.contains("+hello untracked"));
    assert!(output.stderr.is_empty());
    assert!(!repo.path().join(".git/index").exists());
}

#[test]
fn overrides_are_exclusive_and_require_explicit_all_mode() {
    let repo = Repo::new("main");
    for flag in ["-t", "--tracked-only", "-U", "--include-untracked"] {
        for mode in [
            None,
            Some("u"),
            Some("unstaged"),
            Some("s"),
            Some("staged"),
            Some("b"),
            Some("branch"),
            Some("1"),
        ] {
            let mut args = Vec::new();
            args.extend(mode);
            args.push(flag);
            let output = repo.gd(&args);
            assert_eq!(output.status.code(), Some(2), "{args:?}");
            assert!(String::from_utf8_lossy(&output.stderr).contains("all"));
        }
    }
    assert_eq!(repo.gd(&["all", "-t", "-U"]).status.code(), Some(2));
}

#[test]
fn saved_output_is_repeatable_and_keeps_legitimate_diff_files() {
    let repo = Repo::new("main");
    repo.write("source", "source\n");
    use_empty_excludes(&repo);
    repo.write("legitimate.diff", "review this\n");
    repo.gd_in(&["all"]).unwrap();
    let expected = patch(&repo, "uncommitted.diff");
    assert!(String::from_utf8_lossy(&expected).contains("legitimate.diff"));
    for _ in 0..2 {
        repo.gd_in(&["all"]).unwrap();
        assert_eq!(patch(&repo, "uncommitted.diff"), expected);
    }
    let stdout = repo.gd(&["all", "-p"]);
    assert_success(&stdout);
    assert_eq!(stdout.stdout, expected);
}

#[test]
fn redirected_stdout_excludes_only_its_untracked_destination() {
    let repo = Repo::new("main");
    repo.write("source", "source\n");
    let expected = repo.gd(&["all", "-p"]).stdout;
    let path = repo.path().join("arbitrary-output.patch");
    let output = gd_command(repo.path(), &["all"])
        .stdout(Stdio::from(fs::File::create(&path).unwrap()))
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(fs::read(path).unwrap(), expected);
}

#[test]
fn tracked_output_name_remains_visible_after_staged_deletion_and_recreation() {
    let repo = Repo::new("main");
    repo.write("uncommitted.diff", "old content\n");
    repo.git(["add", "-f", "uncommitted.diff"]);
    repo.git(["commit", "-m", "initial"]);
    repo.git(["rm", "uncommitted.diff"]);
    repo.write("uncommitted.diff", "new content\n");
    let output = repo.gd(&["all", "-p"]);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("+new content"));
}

fn use_empty_excludes(repo: &Repo) {
    use std::ffi::OsStr;
    let path = repo.path().join(".git/empty-excludes");
    assert!(path.is_absolute());
    fs::write(&path, "").unwrap();
    repo.git([
        OsStr::new("config"),
        OsStr::new("core.excludesFile"),
        path.as_os_str(),
    ]);
}
