use std::process::Command;

fn gd(args: &[&str]) -> std::process::Output {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    Command::new(env!("CARGO_BIN_EXE_gd"))
        .args(args)
        .current_dir(directory.path())
        .env("HOME", directory.path())
        .env("USERPROFILE", directory.path())
        .env("XDG_CONFIG_HOME", directory.path())
        .env("APPDATA", directory.path())
        .output()
        .expect("gd should run")
}

fn completion(shell: &str) -> std::process::Output {
    gd(&["completions", shell])
}

#[test]
fn generates_each_supported_shell_completion_outside_a_repository() {
    for (shell, marker) in [
        ("bash", "_gd"),
        ("zsh", "#compdef gd"),
        ("fish", "complete -c gd"),
        ("powershell", "Register-ArgumentCompleter"),
        ("elvish", "edit:completion:arg-completer[gd]"),
    ] {
        let output = completion(shell);
        let stdout = String::from_utf8(output.stdout).expect("completion output should be UTF-8");

        assert!(
            output.status.success(),
            "{shell}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!stdout.is_empty(), "{shell}");
        assert!(stdout.contains(marker), "{shell}: {stdout}");
        assert!(
            output.stderr.is_empty(),
            "{shell}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn completion_help_lists_the_required_supported_shells() {
    let output = gd(&["completions", "--help"]);
    let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");

    assert!(output.status.success());
    assert!(stdout.contains("Generate shell completions"));
    assert!(stdout.contains("Usage: gd completions <SHELL>"));
    assert!(stdout.contains("[possible values: bash, elvish, fish, powershell, zsh]"));
    assert!(output.stderr.is_empty());
}

#[test]
fn top_level_help_flag_remains_available() {
    let output = gd(&["--help"]);
    let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");

    assert!(output.status.success());
    assert!(stdout.contains("Usage: gd [OPTIONS] [MODE]"));
    assert!(output.stderr.is_empty());
}

#[test]
fn rejects_the_generated_help_subcommand() {
    for args in [&["help"][..], &["help", "completions"][..]] {
        let output = gd(args);

        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(!output.stderr.is_empty(), "{args:?}");
    }
}

#[test]
fn rejects_diff_arguments_combined_with_completions() {
    for args in [
        &["--header", "completions", "bash"][..],
        &["1", "completions", "bash"][..],
    ] {
        let output = gd(args);

        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(!output.stderr.is_empty(), "{args:?}");
    }
}

#[test]
fn rejects_an_unsupported_completion_shell() {
    let output = completion("cmd");
    let stderr = String::from_utf8(output.stderr).expect("diagnostic should be UTF-8");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr.contains("invalid value 'cmd' for '<SHELL>'"),
        "{stderr}"
    );
    assert!(
        stderr.contains("[possible values: bash, elvish, fish, powershell, zsh]"),
        "{stderr}"
    );
    assert!(output.stdout.is_empty());
}
