mod common;

use std::{ffi::OsString, fs};

use clap::Parser;
use common::{Repo, assert_success, patch};
use git_diff_out::{app, cli::Cli};
use tempfile::tempdir;

fn feature_repo(root: &str) -> Repo {
    let repo = Repo::new(root);
    repo.write("tracked.txt", "root\n");
    repo.commit_all("root");
    repo.git(["switch", "-c", "feature"]);
    repo.write("tracked.txt", "feature\n");
    repo.commit_all("feature");
    repo
}

fn set_remote_default(repo: &Repo, remote: &str, branch: &str, source: &str) {
    repo.git(["remote", "add", remote, repo.path().to_str().unwrap()]);
    let oid = String::from_utf8(repo.git(["rev-parse", source]).stdout).unwrap();
    let tracking = format!("refs/remotes/{remote}/{branch}");
    repo.git(["update-ref", &tracking, oid.trim()]);
    repo.git([
        "symbolic-ref",
        &format!("refs/remotes/{remote}/HEAD"),
        &tracking,
    ]);
}

#[test]
fn branch_aliases_fall_back_to_local_main() {
    let repo = feature_repo("main");
    let expected = repo.git(["diff", "--no-color", "main...HEAD"]).stdout;

    for alias in ["b", "branch"] {
        repo.gd_in(&[alias]).unwrap();
        assert_eq!(patch(&repo, "branch-diff.patch"), expected);
    }
}

#[test]
fn branch_mode_falls_back_to_local_master() {
    let repo = feature_repo("master");
    let expected = repo.git(["diff", "--no-color", "master...HEAD"]).stdout;

    repo.gd_in(&["b"]).unwrap();
    assert_eq!(patch(&repo, "branch-diff.patch"), expected);
}

#[test]
fn explicit_base_supports_custom_root_branch() {
    let repo = feature_repo("develop");
    let expected = repo.git(["diff", "--no-color", "develop...HEAD"]).stdout;

    for alias in ["b", "branch"] {
        let output = repo.gd(&[alias, "develop", "--stdout"]);
        assert_success(&output);
        assert_eq!(output.stdout, expected);
    }
}

#[test]
fn configured_base_precedes_remote_detection() {
    let repo = feature_repo("develop");
    set_remote_default(&repo, "origin", "main", "develop");

    let config_dir = tempdir().unwrap();
    let config_path = config_dir.path().join("config.toml");
    fs::write(&config_path, "base_branch = 'develop'\nquiet = true\n").unwrap();
    let cli = Cli::parse_from(["gd", "b"]);
    app::run_in(
        cli,
        app::Environment {
            cwd: repo.path().to_path_buf(),
            config_path,
            git_program: OsString::from("git"),
        },
    )
    .unwrap();

    let expected = repo.git(["diff", "--no-color", "develop...HEAD"]).stdout;
    assert_eq!(patch(&repo, "branch-diff.patch"), expected);
}

#[test]
fn cli_base_precedes_configured_base() {
    let repo = feature_repo("develop");
    repo.git(["branch", "alternate", "develop"]);
    let config_dir = tempdir().unwrap();
    let config_path = config_dir.path().join("config.toml");
    fs::write(&config_path, "base_branch = 'alternate'\nquiet = true\n").unwrap();
    let cli = Cli::parse_from(["gd", "b", "develop"]);
    app::run_in(
        cli,
        app::Environment {
            cwd: repo.path().to_path_buf(),
            config_path,
            git_program: OsString::from("git"),
        },
    )
    .unwrap();

    let expected = repo.git(["diff", "--no-color", "develop...HEAD"]).stdout;
    assert_eq!(patch(&repo, "branch-diff.patch"), expected);
}

#[test]
fn remote_symbolic_head_selects_remote_tracking_custom_default() {
    let repo = feature_repo("develop");
    set_remote_default(&repo, "company", "develop", "develop");
    repo.git(["branch", "-D", "develop"]);
    let expected = repo
        .git(["diff", "--no-color", "company/develop...HEAD"])
        .stdout;

    repo.gd_in(&["b"]).unwrap();
    assert_eq!(patch(&repo, "branch-diff.patch"), expected);
}

#[test]
fn detached_head_can_use_local_main_fallback() {
    let repo = feature_repo("main");
    repo.git(["checkout", "--detach"]);
    let expected = repo.git(["diff", "--no-color", "main...HEAD"]).stdout;

    repo.gd_in(&["b"]).unwrap();
    assert_eq!(patch(&repo, "branch-diff.patch"), expected);
}

#[test]
fn sole_nonstandard_branch_reports_missing_detectable_base() {
    let repo = Repo::new("feat/initial-cli");
    repo.write("tracked.txt", "first\n");
    repo.commit_all("first");
    repo.write("tracked.txt", "second\n");
    repo.commit_all("second");

    assert_eq!(repo.git(["remote"]).stdout, b"");
    assert_eq!(
        repo.git(["branch", "--format=%(refname:short)"]).stdout,
        b"feat/initial-cli\n"
    );

    let output = repo.gd(&["b"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(
        stderr.contains("no remote default or local main/master branch found"),
        "{stderr}"
    );
    assert!(
        stderr.contains("specify one with 'gd branch <BASE>'"),
        "{stderr}"
    );
    assert!(stderr.contains("configure base_branch"), "{stderr}");
    assert!(!repo.path().join("branch-diff.patch").exists());
}

#[test]
fn remote_setting_without_an_upstream_does_not_override_origin() {
    let repo = Repo::new("develop");
    repo.write("tracked.txt", "root\n");
    repo.commit_all("root");
    repo.git(["branch", "main"]);
    repo.write("tracked.txt", "develop\n");
    repo.commit_all("develop");
    repo.git(["switch", "-c", "feature"]);
    repo.write("feature.txt", "feature\n");
    repo.commit_all("feature");

    set_remote_default(&repo, "origin", "main", "main");
    set_remote_default(&repo, "upstream", "develop", "develop");
    repo.git(["config", "branch.feature.remote", "upstream"]);
    let expected = repo.git(["diff", "--no-color", "main...HEAD"]).stdout;

    repo.gd_in(&["b"]).unwrap();
    assert_eq!(patch(&repo, "branch-diff.patch"), expected);
}

#[test]
fn actual_upstream_remote_precedes_origin() {
    let repo = Repo::new("develop");
    repo.write("tracked.txt", "root\n");
    repo.commit_all("root");
    repo.git(["branch", "main"]);
    repo.write("tracked.txt", "develop\n");
    repo.commit_all("develop");
    repo.git(["switch", "-c", "feature"]);
    repo.write("feature.txt", "feature\n");
    repo.commit_all("feature");

    set_remote_default(&repo, "origin", "main", "main");
    set_remote_default(&repo, "upstream", "develop", "develop");
    let feature_oid = String::from_utf8(repo.git(["rev-parse", "feature"]).stdout).unwrap();
    repo.git([
        "update-ref",
        "refs/remotes/upstream/feature",
        feature_oid.trim(),
    ]);
    repo.git(["branch", "--set-upstream-to", "upstream/feature"]);
    let expected = repo.git(["diff", "--no-color", "develop...HEAD"]).stdout;

    repo.gd_in(&["b"]).unwrap();
    assert_eq!(patch(&repo, "branch-diff.patch"), expected);
}

#[test]
fn remote_default_prefers_a_dangling_equivalent_local_branch() {
    let repo = feature_repo("main");
    set_remote_default(&repo, "origin", "main", "main");
    repo.git(["update-ref", "-d", "refs/remotes/origin/main"]);

    repo.gd_in(&["b"]).unwrap();
    assert_eq!(
        patch(&repo, "branch-diff.patch"),
        repo.git(["diff", "--no-color", "main...HEAD"]).stdout
    );
}
