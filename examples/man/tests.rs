use std::fs;

use clap::{Arg, Command};

use super::{generate, render_manual, validate_argument_docs};

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
            "XDG_CONFIG_HOME",
            "gitrevisions",
        ],
    );
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
fn undocumented_public_argument_is_rejected() {
    let command = Command::new("gd").arg(Arg::new("future").long("future"));
    assert_eq!(
        validate_argument_docs(&command).unwrap_err(),
        "undocumented man-page argument: future"
    );
}

#[test]
fn roff_escapes_control_characters_in_prose() {
    let rendered = super::render_test_section(&[".leading dot", "'leading quote"]);
    assert!(rendered.contains("\\&.leading dot"), "{rendered}");
    assert!(rendered.contains("\\*(Aqleading quote"), "{rendered}");
}
