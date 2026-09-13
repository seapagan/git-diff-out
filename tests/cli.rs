use std::path::PathBuf;

use clap::Parser;
use git_diff_out::cli::{Cli, Mode, QuietOverride};

fn parse(args: &[&str]) -> Cli {
    Cli::try_parse_from(args).expect("arguments should parse")
}

#[test]
fn parses_default_and_unstaged_modes() {
    assert_eq!(parse(&["gd"]).mode().unwrap(), Mode::Default);
    for alias in ["u", "unstaged"] {
        assert_eq!(parse(&["gd", alias]).mode().unwrap(), Mode::Unstaged);
    }
}

#[test]
fn parses_staged_all_and_branch_aliases() {
    for (alias, expected) in [
        ("s", Mode::Staged),
        ("staged", Mode::Staged),
        ("a", Mode::All),
        ("all", Mode::All),
        ("b", Mode::Branch(None)),
        ("branch", Mode::Branch(None)),
    ] {
        assert_eq!(parse(&["gd", alias]).mode().unwrap(), expected);
    }
}

#[test]
fn parses_explicit_branch_base() {
    assert_eq!(
        parse(&["gd", "b", "develop"]).mode().unwrap(),
        Mode::Branch(Some("develop".into()))
    );
    assert_eq!(
        parse(&["gd", "branch", "master"]).mode().unwrap(),
        Mode::Branch(Some("master".into()))
    );
}

#[test]
fn parses_positive_commit_counts() {
    assert_eq!(parse(&["gd", "1"]).mode().unwrap(), Mode::Commits(1));
    assert_eq!(parse(&["gd", "23"]).mode().unwrap(), Mode::Commits(23));
}

#[test]
fn rejects_zero_invalid_modes_and_extra_arguments() {
    for args in [
        &["gd", "0"][..],
        &["gd", "-1"][..],
        &["gd", "wat"][..],
        &["gd", "s", "extra"][..],
        &["gd", "b", "base", "extra"][..],
    ] {
        assert!(Cli::try_parse_from(args).and_then(Cli::validated).is_err());
    }
}

#[test]
fn generates_expected_filenames() {
    for (mode, expected) in [
        (Mode::Default, "diff.patch"),
        (Mode::Unstaged, "unstaged.patch"),
        (Mode::Staged, "staged.patch"),
        (Mode::All, "uncommitted.patch"),
        (Mode::Branch(None), "branch-diff.patch"),
        (Mode::Commits(1), "last-1-commit.patch"),
        (Mode::Commits(4), "last-4-commits.patch"),
    ] {
        assert_eq!(mode.filename(), expected);
    }
}

#[test]
fn parses_delivery_and_quiet_flags() {
    let cli = parse(&["gd", "s", "-o", "patch output", "-q"]);
    assert_eq!(cli.output_dir, Some(PathBuf::from("patch output")));
    assert_eq!(cli.quiet_override(), QuietOverride::Quiet);

    let cli = parse(&["gd", "--verbose"]);
    assert_eq!(cli.quiet_override(), QuietOverride::Verbose);
}

#[test]
fn rejects_conflicting_flags() {
    for args in [
        &["gd", "-p", "-o", "out"][..],
        &["gd", "--stdout", "--output-dir", "out"][..],
        &["gd", "--quiet", "--verbose"][..],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
}
