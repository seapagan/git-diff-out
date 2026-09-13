use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use clap::Parser;
use git_diff_out::{app, cli::Cli};
use tempfile::TempDir;

pub struct Repo {
    pub root: TempDir,
}

impl Repo {
    pub fn new(branch: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let repo = Self { root };
        let output = Command::new("git")
            .arg("init")
            .args(["-b", branch])
            .current_dir(repo.path())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", null_device())
            .output()
            .unwrap();
        assert!(output.status.success());
        repo.git(["config", "user.name", "Test User"]);
        repo.git(["config", "user.email", "test@example.invalid"]);
        repo
    }

    pub fn path(&self) -> &Path {
        self.root.path()
    }

    pub fn write(&self, relative: &str, contents: impl AsRef<[u8]>) {
        let path = self.path().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    pub fn commit_all(&self, message: &str) {
        self.git(["add", "-A"]);
        self.git(["commit", "-m", message]);
    }

    pub fn git<I, S>(&self, args: I) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.path())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", null_device())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    pub fn gd(&self, args: &[&str]) -> Output {
        gd_command(self.path(), args).output().unwrap()
    }

    pub fn gd_in(&self, args: &[&str]) -> Result<Vec<u8>, String> {
        let cli = Cli::try_parse_from(std::iter::once("gd").chain(args.iter().copied()))
            .and_then(Cli::validated)
            .map_err(|error| error.to_string())?;
        let mut messages = Vec::new();
        app::run_in_with_writer(
            cli,
            app::Environment {
                cwd: self.path().to_path_buf(),
                config_path: Some(self.path().join(".gd-test-home/missing-config.toml")),
                git_program: "git".into(),
            },
            &mut messages,
        )
        .map_err(|error| error.to_string())?;
        Ok(messages)
    }
}

pub fn gd_command(cwd: &Path, args: &[&str]) -> Command {
    let isolated = cwd.join(".gd-test-home");
    fs::create_dir_all(&isolated).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_gd"));
    command
        .args(args)
        .current_dir(cwd)
        .env("HOME", &isolated)
        .env("USERPROFILE", &isolated)
        .env("XDG_CONFIG_HOME", &isolated)
        .env("APPDATA", &isolated)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device());
    command
}

pub fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "gd failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn patch(repo: &Repo, name: &str) -> Vec<u8> {
    fs::read(repo.path().join(name)).unwrap()
}

pub fn null_device() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from("NUL")
    } else {
        PathBuf::from("/dev/null")
    }
}
