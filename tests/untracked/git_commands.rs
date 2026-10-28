use super::common::{Repo, assert_success, gd_command};
use std::{fs, os::unix::fs::PermissionsExt, process::Command};

fn wrapped_git(repo: &Repo, args: &[&str]) -> (tempfile::TempDir, Command) {
    let wrapper = tempfile::tempdir().unwrap();
    let git = wrapper.path().join("git");
    fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/untracked-git.sh"
        ),
        &git,
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
    let inherited = std::env::var_os("PATH").unwrap();
    let real_git = std::env::split_paths(&inherited)
        .map(|path| path.join("git"))
        .find(|path| path.is_file())
        .unwrap();
    let paths =
        std::iter::once(wrapper.path().to_path_buf()).chain(std::env::split_paths(&inherited));
    let mut command = gd_command(repo.path(), args);
    command
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("GD_TEST_REAL_GIT", real_git);
    (wrapper, command)
}

#[test]
fn ordinary_untracked_exports_do_not_enumerate_the_revision() {
    for committed in [false, true] {
        let repo = Repo::new("main");
        if committed {
            repo.write("tracked", "base\n");
            repo.commit_all("initial");
        }
        repo.write("new/deep/file", "new content\n");
        let (_wrapper, mut command) = wrapped_git(&repo, &["all", "-p"]);
        let output = command
            .env("GD_TEST_TREE_POLICY", "forbid")
            .output()
            .unwrap();
        assert_success(&output);
        assert!(String::from_utf8_lossy(&output.stdout).contains("+new content"));
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn recreated_tracked_artifacts_use_literal_targeted_revision_lookups() {
    for directory in ["review[1]", ":(glob)review*", "line\nbreak"] {
        let repo = Repo::new("main");
        let name = format!("{directory}/uncommitted.diff");
        repo.write(&name, "old content\n");
        repo.git(["--literal-pathspecs", "add", "-f", &name]);
        repo.git(["commit", "-m", "initial"]);
        repo.git(["--literal-pathspecs", "rm", &name]);
        repo.write(&name, "recreated content\n");
        repo.write("ordinary", "ordinary content\n");
        let index = fs::read(repo.path().join(".git/index")).unwrap();
        let (_wrapper, mut command) = wrapped_git(&repo, &["all", "-o", directory]);
        let output = command
            .env("GD_TEST_TREE_POLICY", "targeted")
            .output()
            .unwrap();
        assert_success(&output);
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains("+recreated content") && text.contains("+ordinary content"));
        assert_eq!(fs::read(repo.path().join(name)).unwrap(), output.stdout);
        assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), index);
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn unsupported_git_versions_fail_without_changing_indexes_or_outputs() {
    let repo = Repo::new("main");
    repo.write("tracked", "before\n");
    repo.commit_all("initial");
    repo.write("new", "new content\n");
    repo.write("uncommitted.diff", "previous valid diff\n");
    let index = fs::read(repo.path().join(".git/index")).unwrap();
    for version in [
        "git version 2.33.9.windows.1",
        "git version 1.9.5.msysgit.1",
        "unknown",
    ] {
        let (_wrapper, mut command) = wrapped_git(&repo, &["all", "-o", "."]);
        let output = command
            .env("GD_TEST_GIT_VERSION", version)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("Git 2.34") && error.contains("--tracked-only"),
            "{error}"
        );
        assert_eq!(fs::read(repo.path().join(".git/index")).unwrap(), index);
        assert_eq!(
            fs::read(repo.path().join("uncommitted.diff")).unwrap(),
            b"previous valid diff\n"
        );
    }
}

#[test]
fn supported_git_version_suffixes_are_accepted() {
    let repo = Repo::new("main");
    repo.write("new", "new content\n");
    for version in [
        "git version 2.34.0",
        "git version 2.34.1.windows.1",
        "git version 2.39.5 (Apple Git-154)",
    ] {
        let (_wrapper, mut command) = wrapped_git(&repo, &["all", "-p"]);
        let output = command
            .env("GD_TEST_GIT_VERSION", version)
            .output()
            .unwrap();
        assert_success(&output);
        assert!(String::from_utf8_lossy(&output.stdout).contains("+new content"));
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn older_git_modes_do_not_probe_the_new_minimum_version() {
    let repo = Repo::new("main");
    repo.write("tracked", "before\n");
    repo.commit_all("initial");
    repo.write("tracked", "committed\n");
    repo.commit_all("second");
    repo.write("tracked", "after\n");
    for args in [
        &["all", "-t", "-p"][..],
        &["-p"],
        &["s", "-p"],
        &["b", "main", "-p"],
        &["1", "-p"],
    ] {
        let (_wrapper, mut command) = wrapped_git(&repo, args);
        let output = command
            .env("GD_TEST_GIT_VERSION", "git version 2.33.0")
            .env("GD_TEST_VERSION_EXIT", "37")
            .output()
            .unwrap();
        assert_success(&output);
        assert!(output.stderr.is_empty());
    }
    let config = if cfg!(target_os = "macos") {
        ".gd-test-home/Library/Application Support/git-diff-out/config.toml"
    } else {
        ".gd-test-home/git-diff-out/config.toml"
    };
    repo.write(config, "[all]\ninclude_untracked = false\n");
    let (_wrapper, mut command) = wrapped_git(&repo, &["all", "-p"]);
    let output = command
        .env("GD_TEST_GIT_VERSION", "git version 2.33.0")
        .env("GD_TEST_VERSION_EXIT", "37")
        .output()
        .unwrap();
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("+after"));
    assert!(output.stderr.is_empty());
}
