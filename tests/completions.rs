use std::process::Command;

#[cfg(unix)]
use std::fs;

#[cfg(unix)]
const BASH_COMPLETION_HARNESS: &str = r#"
source "$1"
completion_spec=($(complete -p "$2"))
for ((i = 0; i < ${#completion_spec[@]}; i++)); do
    if [[ ${completion_spec[i]} == -F ]]; then
        completion_function=${completion_spec[i + 1]}
        break
    fi
done
[[ -n ${completion_function:-} ]] || exit 1
COMP_WORDS=("$2" completions "")
COMP_CWORD=2
"$completion_function" "$2" "" completions
printf '%s\n' "${COMPREPLY[@]}"
"#;

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

#[cfg(unix)]
fn bash_completion_candidates(executable: &str, binary_name: &str) -> Vec<String> {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let script = Command::new(executable)
        .args(["completions", "bash"])
        .current_dir(directory.path())
        .output()
        .expect("completion generator should run");
    assert!(script.status.success());
    assert!(script.stderr.is_empty());

    let script_path = directory.path().join("completion.bash");
    fs::write(&script_path, script.stdout).expect("completion script should be written");
    let output = Command::new("bash")
        .args([
            "--noprofile",
            "--norc",
            "-c",
            BASH_COMPLETION_HARNESS,
            "bash",
        ])
        .arg(&script_path)
        .arg(binary_name)
        .output()
        .expect("bash should run");
    assert!(
        output.status.success(),
        "{binary_name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    String::from_utf8(output.stdout)
        .expect("completion candidates should be UTF-8")
        .lines()
        .filter(|candidate| !candidate.is_empty() && !candidate.starts_with('-'))
        .map(str::to_owned)
        .collect()
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

#[cfg(unix)]
#[test]
fn bash_completes_supported_shells_for_both_public_binary_names() {
    let expected = ["bash", "elvish", "fish", "powershell", "zsh"];

    for (executable, binary_name) in [
        (env!("CARGO_BIN_EXE_gd"), "gd"),
        (env!("CARGO_BIN_EXE_git-diff-out"), "git-diff-out"),
    ] {
        assert_eq!(
            bash_completion_candidates(executable, binary_name),
            expected,
            "{binary_name}"
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
        &["--quiet", "completions", "bash"][..],
        &["--no-header", "completions", "zsh"][..],
        &["--header", "completions", "bash"][..],
    ] {
        let output = gd(args);
        let stderr = String::from_utf8(output.stderr).expect("diagnostic should be UTF-8");

        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(
            stderr.contains(
                "the 'completions' subcommand cannot be combined with diff options or arguments"
            ),
            "{args:?}: {stderr}"
        );
        assert!(
            !stderr.contains("BASE is only valid with branch mode"),
            "{args:?}"
        );
        assert!(!stderr.contains("invalid mode or commit count"), "{args:?}");
    }

    let args = &["1", "completions", "bash"];
    let output = gd(args);
    let stderr = String::from_utf8(output.stderr).expect("diagnostic should be UTF-8");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!stderr.is_empty());
    assert!(
        !stderr.contains("BASE is only valid with branch mode"),
        "{stderr}"
    );
    assert!(!stderr.contains("invalid mode or commit count"), "{stderr}");
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
