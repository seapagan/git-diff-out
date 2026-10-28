use super::common::{Repo, patch};
use clap::Parser;
use git_diff_out::{
    app,
    cli::Cli,
    config::{Config, EffectiveConfig},
};
use std::fs;

#[test]
fn strict_all_configuration_defaults_and_cli_precedence() {
    for document in [
        "",
        "[all]",
        "[all]\ninclude_untracked = true",
        "[all]\ninclude_untracked = false",
    ] {
        let config: Config = toml::from_str(document).unwrap();
        let configured = !document.contains("false");
        for (flag, expected) in [
            (None, configured),
            (Some("-t"), false),
            (Some("--tracked-only"), false),
            (Some("-U"), true),
            (Some("--include-untracked"), true),
        ] {
            let mut args = vec!["gd", "all"];
            args.extend(flag);
            let cli = Cli::try_parse_from(args).and_then(Cli::validated).unwrap();
            assert_eq!(
                EffectiveConfig::new(&config, &cli, std::path::Path::new(".")).include_untracked,
                expected
            );
        }
    }
    for document in [
        "[all]\ninclude_untracked = 'true'",
        "[all]\ninclude_untracked = 1",
        "[all]\nunknown = true",
        "all = false",
    ] {
        assert!(toml::from_str::<Config>(document).is_err());
    }
}

#[test]
fn configured_inclusion_reaches_saved_payload_and_annotations() {
    let repo = Repo::new("main");
    repo.write("tracked", "before\n");
    repo.commit_all("initial");
    repo.write("tracked", "after\n");
    repo.write("untracked", "untracked content\n");
    let config_dir = tempfile::tempdir().unwrap();
    let config = config_dir.path().join("config.toml");
    for enabled in [false, true] {
        fs::write(&config, format!("[all]\ninclude_untracked = {enabled}\n")).unwrap();
        for (flag, expected) in [(None, enabled), (Some("-t"), false), (Some("-U"), true)] {
            let mut args = vec!["gd", "a", "-H", "-q"];
            args.extend(flag);
            app::run_in_with_writer(
                Cli::parse_from(args),
                app::Environment {
                    cwd: repo.path().to_path_buf(),
                    config_path: Some(config.clone()),
                    git_program: "git".into(),
                },
                &mut Vec::new(),
            )
            .unwrap();
            let payload = String::from_utf8(patch(&repo, "uncommitted.diff")).unwrap();
            assert_eq!(payload.contains("+untracked content"), expected);
            assert_eq!(payload.contains("including untracked files"), expected);
            assert_eq!(payload.contains("all tracked changes"), !expected);
        }
    }
}

#[test]
fn cli_override_aliases_include_or_exclude_without_affecting_other_modes() {
    let repo = Repo::new("main");
    repo.write("tracked", "before\n");
    repo.commit_all("initial");
    repo.write("untracked", "new content\n");
    for mode in ["a", "all"] {
        for flag in ["-t", "--tracked-only", "-U", "--include-untracked"] {
            let output = repo.gd(&[mode, flag]);
            super::common::assert_success(&output);
            assert_eq!(
                !output.stdout.is_empty(),
                flag == "-U" || flag == "--include-untracked"
            );
        }
    }
    for args in [
        &[][..],
        &["u"][..],
        &["unstaged"][..],
        &["s"][..],
        &["staged"][..],
    ] {
        let output = repo.gd(args);
        super::common::assert_success(&output);
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn no_header_stdout_still_loads_all_configuration() {
    let repo = Repo::new("main");
    repo.write("new", "new content\n");
    let config_dir = tempfile::tempdir().unwrap();
    let config = config_dir.path().join("config.toml");
    fs::write(&config, "[all]\ninclude_untracked = false\n").unwrap();
    app::run_in_with_writer(
        Cli::parse_from(["gd", "all", "-N", "-p", "-q"]),
        app::Environment {
            cwd: repo.path().to_path_buf(),
            config_path: Some(config),
            git_program: "git".into(),
        },
        &mut Vec::new(),
    )
    .unwrap();
    assert!(!repo.path().join("uncommitted.diff").exists());
}
