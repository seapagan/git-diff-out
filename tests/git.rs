use std::collections::HashSet;

use git_diff_out::{
    cli::Mode,
    git::{choose_base, choose_remote, diff_args},
};

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
