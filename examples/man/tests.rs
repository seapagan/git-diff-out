use std::fs;

use clap::{Arg, Command};

use super::{
    RICH_SUBCOMMANDS, TERSE_SUBCOMMANDS, generate, render_manual, validate_argument_docs,
    validate_subcommand_docs,
};

const SECTIONS: &[&str] = &[
    "NAME",
    "SYNOPSIS",
    "DESCRIPTION",
    "DIFF SPECIFICATIONS",
    "OUTPUT",
    "OPTIONS",
    "SUBCOMMANDS",
    "CONFIGURATION",
    "ENVIRONMENT",
    "EXAMPLES",
    "EXIT STATUS",
    "FILES",
    "SEE ALSO",
];

#[test]
fn generated_manual_is_deterministic_and_structured() {
    let first = render_manual().unwrap();
    let second = render_manual().unwrap();
    assert_eq!(first, second);

    let roff = String::from_utf8(first).unwrap();
    assert!(
        roff.lines().any(|line| line.starts_with(".TH GD 1 ")),
        "{roff}"
    );
    assert_sections(&roff);
    assert_contains_all(
        &roff,
        &[
            "\\-p",
            "\\-\\-stdout",
            "\\-c",
            "\\-\\-copy",
            "\\-C",
            "\\-\\-copy\\-save",
            "\\-\\-output\\-dir",
            "\\-\\-header",
            "\\-\\-no\\-header",
            "\\-\\-note",
            "\\-\\-quiet",
            "\\-\\-verbose",
            "\\-\\-help",
            "\\-\\-version",
            "MODE",
            "BASE",
            "completions",
            "bash",
            "powershell",
        ],
    );
    assert_contains_all(
        &roff,
        &[
            "An explicit BASE takes precedence",
            "With non\\-terminal stdout",
            "Every selected destination receives the header",
            "when file output is active",
            "absolute path",
            "XDG_CONFIG_HOME",
            "gitrevisions",
        ],
    );
    assert!(!roff.contains("gd\\-completions(1)"), "{roff}");
}

fn assert_sections(roff: &str) {
    let mut previous = 0;
    for section in SECTIONS {
        let marker = if section.contains(' ') {
            format!(".SH \"{section}\"")
        } else {
            format!(".SH {section}")
        };
        let position = roff
            .find(&marker)
            .unwrap_or_else(|| panic!("missing {marker}"));
        assert!(position >= previous, "section order: {section}");
        assert_eq!(roff.matches(&marker).count(), 1, "duplicate {section}");
        previous = position;
    }
}

fn assert_contains_all(roff: &str, expected: &[&str]) {
    for text in expected {
        assert!(roff.contains(text), "missing {text}");
    }
}

#[test]
fn generation_creates_the_parent_and_exact_page() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("nested/gd.1");
    generate(&path).unwrap();
    assert_eq!(fs::read(path).unwrap(), render_manual().unwrap());
}

#[test]
fn manual_qualifies_when_configuration_errors_apply() {
    let roff = String::from_utf8(render_manual().unwrap()).unwrap();

    assert_contains_all(
        &roff,
        &[
            "when an invocation needs configuration",
            "unreadable files, invalid TOML, and unknown keys cause an error",
            "With \\-\\-no\\-header and stdout as the only destination",
            "skips configuration unless branch mode needs it to select a base",
        ],
    );
}

#[test]
fn manual_describes_the_selected_file_directory() {
    let roff = String::from_utf8(render_manual().unwrap()).unwrap();

    assert_contains_all(
        &roff,
        &[
            "The default directory is the current directory",
            "configured output_dir or \\-\\-output\\-dir selects another",
            "save unstaged.diff under the selected output directory",
        ],
    );
}

#[test]
fn manual_describes_automatic_branch_base_selection_in_order() {
    let roff = String::from_utf8(render_manual().unwrap()).unwrap();
    let expected = [
        "current branch\\*(Aqs upstream remote when that remote exists",
        "origin when present",
        "the sole configured remote",
        "matching local branch when it exists",
        "remote\\-tracking ref",
        "local main, then local master",
        "reports an error when none of these choices yields a base",
    ];
    let mut previous = 0;

    for text in expected {
        let position = roff.find(text).unwrap_or_else(|| panic!("missing {text}"));
        assert!(position >= previous, "out of order: {text}");
        previous = position;
    }
}

#[test]
fn undocumented_public_argument_is_rejected() {
    let command = Command::new("gd").arg(Arg::new("future").long("future"));
    assert_eq!(
        validate_argument_docs(&command).unwrap_err(),
        "undocumented man-page argument: future"
    );
}

#[test]
fn undocumented_public_subcommand_is_rejected() {
    let command = Command::new("gd")
        .subcommand(Command::new("completions"))
        .subcommand(Command::new("future"));

    assert_eq!(
        validate_subcommand_docs(&command, RICH_SUBCOMMANDS, TERSE_SUBCOMMANDS).unwrap_err(),
        "undocumented man-page subcommand: future"
    );
}

#[test]
fn missing_documented_subcommand_is_rejected() {
    assert_eq!(
        validate_subcommand_docs(&Command::new("gd"), RICH_SUBCOMMANDS, TERSE_SUBCOMMANDS,)
            .unwrap_err(),
        "man-page documentation refers to missing subcommand: completions"
    );
}

#[test]
fn conflicting_subcommand_classifications_are_rejected() {
    let command = Command::new("gd").subcommand(Command::new("completions"));

    assert_eq!(
        validate_subcommand_docs(&command, RICH_SUBCOMMANDS, &["completions"]).unwrap_err(),
        "subcommand has both rich and terse man-page documentation: completions"
    );
}

#[test]
fn classified_terse_subcommand_is_rendered_from_live_metadata() {
    let command =
        Command::new("gd").subcommand(Command::new("future").about("Inspect future state"));
    let terse = &["future"];
    validate_subcommand_docs(&command, &[], terse).unwrap();
    let mut output = Vec::new();

    super::content::render_subcommands_with_docs(&command, &[], terse, &mut output).unwrap();

    let roff = String::from_utf8(output).unwrap();
    assert_contains_all(
        &roff,
        &[".SH SUBCOMMANDS", "\\fBfuture\\fR", "Inspect future state"],
    );
}

#[test]
fn roff_escapes_control_characters_in_prose() {
    let rendered = super::render_test_section(&[".leading dot", "'leading quote"]);
    assert!(rendered.contains("\\&.leading dot"), "{rendered}");
    assert!(rendered.contains("\\*(Aqleading quote"), "{rendered}");
}
