use std::{path::PathBuf, process::Command};

use clap::{CommandFactory, Parser};
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
fn rejects_base_outside_branch_mode() {
    for args in [
        &["gd", "s", "main"][..],
        &["gd", "staged", "develop"][..],
        &["gd", "u", "master"][..],
        &["gd", "a", "main"][..],
        &["gd", "3", "develop"][..],
    ] {
        let error = Cli::try_parse_from(args)
            .and_then(Cli::validated)
            .expect_err("BASE outside branch mode should be rejected");
        assert!(
            error
                .to_string()
                .contains("BASE is only valid with branch mode")
        );
    }
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
fn binary_reports_application_mode_validation_errors() {
    let output = Command::new(env!("CARGO_BIN_EXE_gd"))
        .arg("wat")
        .output()
        .expect("gd should run");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr.contains("invalid mode or commit count 'wat'"),
        "{stderr}"
    );
}

#[test]
fn generates_expected_filenames() {
    for (mode, expected) in [
        (Mode::Default, "diff.patch"),
        (Mode::Unstaged, "unstaged.patch"),
        (Mode::Staged, "staged.patch"),
        (Mode::All, "uncommitted.patch"),
        (Mode::Branch(None), "branch-diff.patch"),
        (Mode::Commits(1), "last-commit.patch"),
        (Mode::Commits(10), "last-10-commits.patch"),
    ] {
        assert_eq!(mode.filename(), expected);
    }
}

#[test]
fn parses_delivery_and_quiet_flags() {
    let cli = parse(&["gd", "s", "-o", "patch output", "-q"]);
    assert_eq!(cli.output_dir, Some(PathBuf::from("patch output")));
    assert_eq!(cli.quiet_override(), QuietOverride::Quiet);

    for flag in ["-v", "--verbose"] {
        let cli = parse(&["gd", flag]);
        assert_eq!(cli.quiet_override(), QuietOverride::Verbose);
    }
}

#[test]
fn rejects_conflicting_flags() {
    for args in [
        &["gd", "-p", "-o", "out"][..],
        &["gd", "--stdout", "--output-dir", "out"][..],
        &["gd", "-q", "-v"][..],
        &["gd", "--quiet", "--verbose"][..],
        &["gd", "-q", "--verbose"][..],
        &["gd", "--quiet", "-v"][..],
        &["gd", "-vv"][..],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
}

#[test]
fn help_describes_the_cli_grammar_and_generated_options() {
    let help = Cli::command().render_help().to_string();

    assert!(help.contains("Usage: gd [OPTIONS] [MODE]\n"));
    assert!(help.contains("  gd [OPTIONS] {b|branch} [BASE]\n"));
    assert!(help.contains("[MODE]  Diff mode:"));
    assert!(help.contains("-v, --verbose"));
    assert!(help.contains("-V, --version"));
    assert!(help.contains("Examples:"));
    assert!(help.contains("gd                 Write unstaged tracked changes"));
    assert!(help.contains("gd s               Write staged tracked changes"));
    assert!(help.contains("gd 3"));
    assert!(help.contains("gd b"));
    assert!(help.contains("gd b develop"));
    assert!(help.contains("gd s -p"));
    assert!(help.contains("current-branch changes relative to"));
}

#[test]
fn captured_help_is_plain_text() {
    let output = Command::new(env!("CARGO_BIN_EXE_gd"))
        .arg("--help")
        .output()
        .expect("help should run");
    let help = String::from_utf8(output.stdout).expect("help should be UTF-8");

    assert!(output.status.success());
    assert!(help.contains("Examples:"));
    assert!(!help.contains('\u{1b}'));
}
