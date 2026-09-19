use std::{fs, path::PathBuf};

use clap::Parser;
use git_diff_out::{
    cli::Cli,
    config::{Config, ConfigError, EffectiveConfig},
};
use tempfile::tempdir;

#[test]
fn missing_config_uses_defaults() {
    let temp = tempdir().unwrap();
    let config = Config::load(&temp.path().join("missing.toml")).unwrap();

    assert_eq!(config.output_dir, PathBuf::from("."));
    assert!(!config.quiet);
    assert_eq!(config.base_branch, None);
    assert!(!config.clipboard.osc52_fallback);
    assert!(!config.header.enabled);
    assert_eq!(config.header.note, None);
}

#[test]
fn loads_local_osc52_fallback_setting() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("config.toml");
    fs::write(&path, "[clipboard]\nosc52_fallback = true\n").unwrap();

    assert!(Config::load(&path).unwrap().clipboard.osc52_fallback);
}

#[test]
fn loads_optional_header_settings() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("config.toml");
    fs::write(
        &path,
        "[header]\nenabled = true\nnote = 'Review error handling carefully'\n",
    )
    .unwrap();

    let config = Config::load(&path).unwrap();
    assert!(config.header.enabled);
    assert_eq!(
        config.header.note.as_deref(),
        Some("Review error handling carefully")
    );
}

#[test]
fn loads_supported_config_values() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("config.toml");
    fs::write(
        &path,
        "output_dir = 'review patches'\nquiet = true\nbase_branch = 'develop'\n",
    )
    .unwrap();

    let config = Config::load(&path).unwrap();
    assert_eq!(config.output_dir, PathBuf::from("review patches"));
    assert!(config.quiet);
    assert_eq!(config.base_branch.as_deref(), Some("develop"));
}

#[test]
fn rejects_unknown_config_values() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("config.toml");
    fs::write(&path, "context_lines = 8\n").unwrap();

    let error = Config::load(&path).unwrap_err().to_string();
    assert!(error.contains("context_lines"), "{error}");
}

#[test]
fn config_read_errors_include_the_path_and_cause() {
    let temp = tempdir().unwrap();
    let error = Config::load(temp.path()).unwrap_err().to_string();

    assert!(error.contains("cannot read config"), "{error}");
    assert!(
        error.contains(&temp.path().display().to_string()),
        "{error}"
    );
}

#[test]
fn missing_config_directory_error_is_actionable() {
    assert_eq!(
        ConfigError::NoConfigDirectory.to_string(),
        "cannot determine the config directory"
    );
}

#[test]
fn resolves_relative_and_absolute_output_paths() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().to_path_buf();
    let relative = Config {
        output_dir: PathBuf::from("review patches"),
        ..Config::default()
    };
    let absolute = Config {
        output_dir: cwd.join("elsewhere"),
        ..Config::default()
    };

    assert_eq!(
        EffectiveConfig::new(&relative, &Cli::parse_from(["gd"]), &cwd).output_dir,
        cwd.join("review patches")
    );
    assert_eq!(
        EffectiveConfig::new(&absolute, &Cli::parse_from(["gd"]), &cwd).output_dir,
        cwd.join("elsewhere")
    );
}

#[test]
fn cli_output_directory_overrides_config() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().to_path_buf();
    let config = Config {
        output_dir: PathBuf::from("configured"),
        ..Config::default()
    };
    let cli = Cli::parse_from(["gd", "-o", "command line"]);

    assert_eq!(
        EffectiveConfig::new(&config, &cli, &cwd).output_dir,
        cwd.join("command line")
    );
}

#[test]
fn quiet_and_verbose_override_config() {
    let temp = tempdir().unwrap();
    let cwd = temp.path().to_path_buf();
    let quiet_config = Config {
        quiet: true,
        ..Config::default()
    };

    assert!(EffectiveConfig::new(&quiet_config, &Cli::parse_from(["gd"]), &cwd).quiet);
    assert!(
        !EffectiveConfig::new(&quiet_config, &Cli::parse_from(["gd", "--verbose"]), &cwd).quiet
    );
    assert!(
        EffectiveConfig::new(
            &Config::default(),
            &Cli::parse_from(["gd", "--quiet"]),
            &cwd
        )
        .quiet
    );
}
