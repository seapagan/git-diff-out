use super::common::{Repo, assert_success};
use std::fs;

fn nested_repository(repo: &Repo, committed: bool, gitfile: bool) {
    let mut args = vec!["init", "-b", "main"];
    if gitfile {
        args.extend(["--separate-git-dir", ".git/nested-gitdir"]);
    }
    args.push("nested");
    repo.git(args);
    repo.write("nested/content", "nested content\n");
    if committed {
        repo.git(["-C", "nested", "add", "content"]);
        repo.git([
            "-C",
            "nested",
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-m",
            "nested",
        ]);
    }
}

#[test]
fn untracked_nested_repositories_are_excluded_without_warnings_or_index_changes() {
    for committed in [false, true] {
        for gitfile in [false, true] {
            let repo = Repo::new("main");
            repo.write("tracked", "before\n");
            repo.commit_all("initial");
            nested_repository(&repo, committed, gitfile);
            repo.write("tracked", "after\n");
            repo.write("ordinary/deep/new", "ordinary content\n");
            let index = fs::read(repo.path().join(".git/index")).unwrap();
            let output = repo.gd(&["all", "-p"]);
            assert_success(&output);
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.contains("+after") && text.contains("+ordinary content"));
            assert!(!text.contains("nested") && !text.contains("160000"));
            assert!(output.stderr.is_empty(), "{:?}", output.stderr);
            assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), index);
        }
    }
}

#[test]
fn tracked_submodules_remain_in_the_export() {
    let repo = Repo::new("main");
    nested_repository(&repo, true, true);
    repo.git(["add", "nested"]);
    repo.git(["commit", "-m", "tracked submodule"]);
    repo.write("nested/content", "changed submodule\n");
    repo.write("ordinary/new", "ordinary content\n");
    let index = fs::read(repo.path().join(".git/index")).unwrap();
    let output = repo.gd(&["all", "-p"]);
    assert_success(&output);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("b/nested") && text.contains("-dirty"),
        "{text}"
    );
    assert!(text.contains("+ordinary content"));
    assert!(output.stderr.is_empty());
    assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), index);
}

#[cfg(unix)]
#[test]
fn symlinks_to_nested_repositories_remain_ordinary_untracked_files() {
    let repo = Repo::new("main");
    nested_repository(&repo, false, false);
    std::os::unix::fs::symlink("nested", repo.path().join("link")).unwrap();
    let output = repo.gd(&["all", "-p"]);
    assert_success(&output);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("new file mode 120000") && text.contains("+nested"));
    assert!(!text.contains("b/nested"));
    assert!(output.stderr.is_empty());
}
