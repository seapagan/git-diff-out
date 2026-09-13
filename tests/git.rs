#[cfg(unix)]
#[path = "support/fake_program.rs"]
mod fake_program;

use std::{collections::HashSet, ffi::OsStr};

#[cfg(unix)]
use fake_program::executable_script;

#[cfg(unix)]
use git_diff_out::git::detect_base;
use git_diff_out::{
    cli::Mode,
    git::{choose_base, choose_remote, diff_args, resolved_diff_args},
};
use tempfile::tempdir;

#[test]
fn builds_exact_diff_arguments() {
    for (mode, base, expected) in [
        (Mode::Default, None, vec!["diff", "--no-color"]),
        (Mode::Unstaged, None, vec!["diff", "--no-color"]),
        (Mode::Staged, None, vec!["diff", "--no-color", "--staged"]),
        (Mode::All, None, vec!["diff", "--no-color", "HEAD"]),
        (
            Mode::Branch(None),
            Some("develop"),
            vec!["diff", "--no-color", "develop...HEAD"],
        ),
        (
            Mode::Commits(3),
            None,
            vec!["diff", "--no-color", "HEAD~3..HEAD"],
        ),
    ] {
        assert_eq!(diff_args(&mode, base), expected);
    }
}

#[test]
fn upstream_remote_precedes_origin_and_sole_remote() {
    let remotes = vec!["origin".into(), "upstream".into()];
    assert_eq!(
        choose_remote(Some("upstream"), &remotes).as_deref(),
        Some("upstream")
    );
}

#[test]
fn origin_precedes_other_remotes_without_upstream() {
    let remotes = vec!["backup".into(), "origin".into()];
    assert_eq!(choose_remote(None, &remotes).as_deref(), Some("origin"));
}

#[test]
fn sole_remote_is_used_without_upstream_or_origin() {
    let remotes = vec!["company".into()];
    assert_eq!(choose_remote(None, &remotes).as_deref(), Some("company"));
    assert_eq!(choose_remote(None, &["one".into(), "two".into()]), None);
}

#[test]
fn remote_default_prefers_equivalent_local_branch() {
    let locals = HashSet::from(["main".to_owned()]);
    assert_eq!(
        choose_base(Some(("origin", "origin/main")), &locals).as_deref(),
        Some("main")
    );
}

#[test]
fn remote_default_uses_tracking_ref_without_local_branch() {
    assert_eq!(
        choose_base(Some(("origin", "origin/develop")), &HashSet::new()).as_deref(),
        Some("origin/develop")
    );
}

#[test]
fn local_main_then_master_are_fallbacks() {
    let both = HashSet::from(["main".to_owned(), "master".to_owned()]);
    assert_eq!(choose_base(None, &both).as_deref(), Some("main"));
    assert_eq!(
        choose_base(None, &HashSet::from(["master".to_owned()])).as_deref(),
        Some("master")
    );
    assert_eq!(choose_base(None, &HashSet::new()), None);
}

#[test]
fn mismatched_symbolic_remote_is_ignored() {
    let locals = HashSet::from(["master".to_owned()]);
    assert_eq!(
        choose_base(Some(("origin", "upstream/main")), &locals).as_deref(),
        Some("master")
    );
}

#[test]
fn all_mode_reports_git_startup_failure() {
    let cwd = tempdir().unwrap();
    let error = resolved_diff_args(
        &Mode::All,
        None,
        cwd.path(),
        OsStr::new("git-executable-that-does-not-exist"),
    )
    .unwrap_err();

    assert!(error.contains("failed to start git"), "{error}");
}

#[cfg(unix)]
#[test]
fn all_mode_reports_empty_tree_command_failure() {
    let cwd = tempdir().unwrap();
    let (_program_dir, program) = executable_script("hash-fails");
    let error = resolved_diff_args(&Mode::All, None, cwd.path(), program.as_os_str()).unwrap_err();

    assert_eq!(error, "cannot resolve Git's empty tree: hash failed");
}

#[cfg(unix)]
#[test]
fn all_mode_rejects_an_empty_tree_object_id() {
    let cwd = tempdir().unwrap();
    let (_program_dir, program) = executable_script("empty-tree-empty");
    let error = resolved_diff_args(&Mode::All, None, cwd.path(), program.as_os_str()).unwrap_err();

    assert_eq!(error, "git returned an empty empty-tree object ID");
}

#[cfg(unix)]
#[test]
fn all_mode_rejects_non_utf8_empty_tree_object_id() {
    let cwd = tempdir().unwrap();
    let (_program_dir, program) = executable_script("empty-tree-non-utf8");
    let error = resolved_diff_args(&Mode::All, None, cwd.path(), program.as_os_str()).unwrap_err();

    assert_eq!(error, "git returned a non-UTF-8 empty-tree object ID");
}

#[cfg(unix)]
#[test]
fn all_mode_reports_hash_object_startup_failure() {
    let cwd = tempdir().unwrap();
    let (_program_dir, program) = executable_script("hash-startup-fails");
    let error = resolved_diff_args(&Mode::All, None, cwd.path(), program.as_os_str()).unwrap_err();

    assert!(error.contains("failed to start git"), "{error}");
}

#[cfg(unix)]
#[test]
fn base_detection_reports_a_mid_sequence_startup_failure() {
    let cwd = tempdir().unwrap();
    let (_program_dir, program) = executable_script("base-mid-startup-fails");
    let error = detect_base(cwd.path(), program.as_os_str()).unwrap_err();

    assert!(error.contains("failed to start git"), "{error}");
}

#[cfg(unix)]
#[test]
fn base_detection_rejects_non_utf8_reference_data() {
    let cwd = tempdir().unwrap();
    let (_program_dir, program) = executable_script("reference-non-utf8");
    let error = detect_base(cwd.path(), program.as_os_str()).unwrap_err();

    assert_eq!(error, "git returned non-UTF-8 reference data");
}
