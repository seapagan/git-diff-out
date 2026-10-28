use super::common;
use common::{Repo, assert_success, gd_command};
use std::fs;

#[test]
fn sparse_checkout_includes_outside_cone_without_changing_index_or_configuration() {
    let repo = Repo::new("main");
    repo.write("inside/tracked", "before\n");
    repo.write("outside/tracked", "outside\n");
    repo.write(".gitignore", "*.ignored\n");
    repo.commit_all("initial");
    repo.git([
        "sparse-checkout",
        "set",
        "--cone",
        "--sparse-index",
        "inside",
    ]);
    repo.write("inside/tracked", "after\n");
    repo.write("outside/new", "out of cone\n");
    repo.write("outside/hidden.ignored", "ignored\n");
    let preserved = ["index", "config", "info/sparse-checkout"];
    let before = preserved.map(|name| fs::read(repo.path().join(".git").join(name)).unwrap());
    let output = repo.gd(&["all", "-p"]);
    assert_success(&output);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("b/inside/tracked") && text.contains("+after"));
    assert!(text.contains("b/outside/new") && text.contains("+out of cone"));
    assert!(!text.contains("hidden.ignored"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("diff --git"));
    assert!(!repo.path().join("uncommitted.diff").exists());
    assert_eq!(
        preserved.map(|name| fs::read(repo.path().join(".git").join(name)).unwrap()),
        before
    );
}

#[test]
fn relative_git_paths_from_subdirectory_preserve_indexes_and_clean_up() {
    for committed in [false, true] {
        for alternate in [false, true] {
            let repo = Repo::new("main");
            repo.write("tracked", "before\n");
            repo.git(["add", "tracked"]);
            if committed {
                repo.git(["commit", "-m", "initial"]);
            }
            repo.write("tracked", "after\n");
            repo.write("new", "root content\n");
            repo.write("subdir/new", "nested content\n");
            let index = fs::read(repo.path().join(".git/index")).unwrap();
            if alternate {
                repo.write(".git/alternate-index", &index);
            }
            let temporary = tempfile::tempdir().unwrap();
            let mut command = gd_command(&repo.path().join("subdir"), &["all", "-p"]);
            command.env("GIT_DIR", "../.git").env("GIT_WORK_TREE", "..");
            if alternate {
                // Git resolves the index path after entering its work tree.
                command.env("GIT_INDEX_FILE", ".git/alternate-index");
            }
            let output = command
                .env("TMPDIR", temporary.path())
                .env("TMP", temporary.path())
                .env("TEMP", temporary.path())
                .output()
                .unwrap();
            assert_success(&output);
            let text = String::from_utf8(output.stdout).unwrap();
            for content in ["+after", "+root content", "+nested content"] {
                assert!(text.contains(content), "{text}");
            }
            assert!(output.stderr.is_empty());
            assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), index);
            if alternate {
                assert_eq!(
                    fs::read(repo.path().join(".git/alternate-index")).unwrap(),
                    index
                );
            }
            assert_eq!(fs::read_dir(temporary.path()).unwrap().count(), 0);
            assert!(!repo.path().join("subdir/uncommitted.diff").exists());
        }
    }
}

#[test]
fn relative_git_paths_work_without_an_existing_index() {
    let repo = Repo::new("main");
    repo.write("new", "root content\n");
    repo.write("subdir/new", "nested content\n");
    let output = gd_command(&repo.path().join("subdir"), &["all", "-p"])
        .env("GIT_DIR", "../.git")
        .env("GIT_WORK_TREE", "..")
        .env("GIT_INDEX_FILE", ".git/missing-index")
        .output()
        .unwrap();
    assert_success(&output);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("+root content") && text.contains("+nested content"));
    assert!(output.stderr.is_empty());
    assert!(!repo.path().join(".git/index").exists());
    assert!(!repo.path().join(".git/missing-index").exists());
}
