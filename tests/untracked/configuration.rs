use super::common::{Repo, assert_success, gd_command, patch};
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
                String::from_utf8_lossy(&output.stdout).contains("+new content"),
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
    if let Some(config) = std::env::var_os("GD_TEST_ALL_CONFIG") {
        no_header_stdout_child(config.into());
        return;
    }
    let repo = Repo::new("main");
    repo.write("tracked", "before\n");
    repo.commit_all("initial");
    repo.write("tracked", "after\n");
    repo.write("new", "new content\n");
    let config_dir = tempfile::tempdir().unwrap();
    let config = config_dir.path().join("config.toml");
    for enabled in [false, true] {
        fs::write(
            &config,
            format!("[all]\ninclude_untracked = {enabled}\n[header]\nenabled = true\n"),
        )
        .unwrap();
        let payload = no_header_stdout_payload(&repo, &config);
        assert_eq!(payload.contains("+new content"), enabled);
        assert!(payload.contains("+after"));
        assert!(payload.starts_with("diff --git "));
        assert!(!repo.path().join("uncommitted.diff").exists());
    }
}

fn no_header_stdout_payload(repo: &Repo, config: &std::path::Path) -> String {
    let isolated = gd_command(repo.path(), &[]);
    let executable = std::path::absolute(std::env::args_os().next().unwrap()).unwrap();
    let output = std::process::Command::new(executable)
        .envs(
            isolated
                .get_envs()
                .filter_map(|(key, value)| value.map(|value| (key, value))),
        )
        .current_dir(repo.path())
        .args([
            "--exact",
            "configuration::no_header_stdout_still_loads_all_configuration",
            "--nocapture",
        ])
        .env("GD_TEST_ALL_CONFIG", config)
        .output()
        .unwrap();
    assert_success(&output);
    assert!(output.stderr.is_empty());
    // Delimit application stdout from the child test harness's own output.
    String::from_utf8(output.stdout)
        .unwrap()
        .split_once("GD_PAYLOAD_BEGIN\n")
        .unwrap()
        .1
        .split_once("GD_PAYLOAD_END\n")
        .unwrap()
        .0
        .to_owned()
}

fn no_header_stdout_child(config: std::path::PathBuf) {
    use std::io::Write;
    println!("GD_PAYLOAD_BEGIN");
    std::io::stdout().flush().unwrap();
    app::run_in_with_writer(
        Cli::parse_from(["gd", "all", "-N", "-p", "-q"]),
        app::Environment {
            cwd: std::env::current_dir().unwrap(),
            config_path: Some(config),
            git_program: "git".into(),
        },
        &mut Vec::new(),
    )
    .unwrap();
    println!("GD_PAYLOAD_END");
}
