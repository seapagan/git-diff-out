mod common;

use std::fs;

#[cfg(windows)]
use clap::Parser;
use common::{Repo, assert_success, patch};
#[cfg(windows)]
use git_diff_out::{app, cli::Cli};

#[cfg(windows)]
const CONFIG_PATH: &str = ".git/gd-config.toml";
#[cfg(target_os = "macos")]
const CONFIG_PATH: &str = ".gd-test-home/Library/Application Support/git-diff-out/config.toml";
#[cfg(not(any(windows, target_os = "macos")))]
const CONFIG_PATH: &str = ".gd-test-home/git-diff-out/config.toml";

fn changed_repo() -> Repo {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "before\n");
    repo.commit_all("initial");
    repo.write("tracked.txt", "after\n");
    repo
}

fn configure(repo: &Repo, contents: &str) {
    repo.write(CONFIG_PATH, contents);
}

fn add_origin(repo: &Repo, url: &str) {
    repo.git(["remote", "add", "origin", url]);
}

fn repo_with_changes_for_all_modes() -> Repo {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "initial\n");
    repo.commit_all("initial");
    repo.write("tracked.txt", "main change\n");
    repo.commit_all("main change");
    repo.git(["switch", "-c", "feature"]);
    repo.write("tracked.txt", "feature change\n");
    repo.commit_all("feature change");
    repo.write("tracked.txt", "staged change\n");
    repo.git(["add", "tracked.txt"]);
    repo.write("tracked.txt", "unstaged change\n");
    add_origin(&repo, "git@github.com:seapagan/gd.git");
    repo
}

fn expected_header(contents: &str, repository: &str, note: Option<&str>, diff: &[u8]) -> Vec<u8> {
    let mut expected = format!("# contents: {contents}\n# repository: {repository}\n");
    if let Some(note) = note {
        expected.push_str(&format!("# note: {note}\n"));
    }
    expected.push('\n');
    let mut expected = expected.into_bytes();
    expected.extend_from_slice(diff);
    expected
}

fn assert_stdout(repo: &Repo, args: &[&str], expected: &[u8]) {
    #[cfg(windows)]
    if repo.path().join(CONFIG_PATH).exists() {
        let mut args = args.to_vec();
        args.extend(["--output-dir", ".gd-test-output", "--quiet"]);
        assert!(run_with_config(repo, &args).is_empty());
        assert_eq!(patch(repo, ".gd-test-output/unstaged.diff"), expected);
        return;
    }

    let output = repo.gd(args);
    assert_success(&output);
    assert_eq!(output.stdout, expected);
    assert!(output.stderr.is_empty());
}

#[cfg(windows)]
fn run_with_config(repo: &Repo, args: &[&str]) -> Vec<u8> {
    let cli = Cli::try_parse_from(std::iter::once("gd").chain(args.iter().copied()))
        .and_then(Cli::validated)
        .unwrap();
    let mut messages = Vec::new();
    app::run_in_with_writer(
        cli,
        app::Environment {
            cwd: repo.path().to_path_buf(),
            config_path: Some(repo.path().join(CONFIG_PATH)),
            git_program: "git".into(),
        },
        &mut messages,
    )
    .unwrap();
    messages
}

#[test]
fn default_output_remains_header_free() {
    let repo = changed_repo();
    let diff = repo.git(["diff", "--no-color"]).stdout;

    assert_stdout(&repo, &[], &diff);
}

#[test]
fn config_and_cli_control_header_enablement() {
    let repo = changed_repo();
    add_origin(&repo, "git@github.com:seapagan/gd.git");
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let annotated = expected_header(
        "Git diff of unstaged changes",
        "seapagan/gd",
        Some("Review error handling carefully"),
        &diff,
    );
    configure(
        &repo,
        "[header]\nenabled = true\nnote = 'Review error handling carefully'\n",
    );

    assert_stdout(&repo, &[], &annotated);
    assert_stdout(&repo, &["--header"], &annotated);
    assert_stdout(&repo, &["--no-header"], &diff);
}

#[test]
fn cli_note_enables_header_and_overrides_configured_note() {
    let repo = changed_repo();
    add_origin(&repo, "https://github.com/seapagan/gd.git");
    configure(
        &repo,
        "[header]\nenabled = false\nnote = 'Configured note'\n",
    );
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header(
        "Git diff of unstaged changes",
        "seapagan/gd",
        Some("CLI note"),
        &diff,
    );

    assert_stdout(&repo, &["--note", "CLI note"], &expected);
}

#[test]
fn blank_cli_notes_enable_header_without_a_note_field() {
    let repo = changed_repo();
    add_origin(&repo, "https://github.com/seapagan/gd.git");
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header("Git diff of unstaged changes", "seapagan/gd", None, &diff);

    for note in ["", "   ", "\t", "\n\r\n"] {
        assert_stdout(&repo, &["--note", note], &expected);
    }
}

#[test]
fn blank_configured_notes_are_omitted_from_enabled_headers() {
    let repo = changed_repo();
    add_origin(&repo, "https://github.com/seapagan/gd.git");
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header("Git diff of unstaged changes", "seapagan/gd", None, &diff);

    for config in [
        "[header]\nenabled = true\nnote = ''\n",
        "[header]\nenabled = true\nnote = \" \\t\\n\\n\"\n",
    ] {
        configure(&repo, config);
        assert_stdout(&repo, &[], &expected);
    }
}

#[test]
fn empty_cli_note_suppresses_configured_note_and_enables_header() {
    let repo = changed_repo();
    add_origin(&repo, "https://github.com/seapagan/gd.git");
    configure(
        &repo,
        "[header]\nenabled = false\nnote = 'Configured note'\n",
    );
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header("Git diff of unstaged changes", "seapagan/gd", None, &diff);

    assert_stdout(&repo, &["--note", ""], &expected);
}

#[test]
fn configured_note_appears_only_with_enabled_header() {
    let repo = changed_repo();
    add_origin(&repo, "https://github.com/seapagan/gd.git");
    configure(&repo, "[header]\nnote = 'Dormant note'\n");
    let diff = repo.git(["diff", "--no-color"]).stdout;

    assert_stdout(&repo, &[], &diff);
    let expected = expected_header(
        "Git diff of unstaged changes",
        "seapagan/gd",
        Some("Dormant note"),
        &diff,
    );
    assert_stdout(&repo, &["--header"], &expected);
}

#[test]
fn header_uses_exact_contents_text_for_every_mode() {
    let repo = repo_with_changes_for_all_modes();

    for (args, contents, git_args) in [
        (
            &["--header"][..],
            "Git diff of unstaged changes",
            &["diff", "--no-color"][..],
        ),
        (
            &["unstaged", "--header"][..],
            "Git diff of unstaged changes",
            &["diff", "--no-color"][..],
        ),
        (
            &["staged", "--header"][..],
            "Git diff of staged changes",
            &["diff", "--no-color", "--staged"][..],
        ),
        (
            &["all", "--header"][..],
            "Git diff of all tracked changes",
            &["diff", "--no-color", "HEAD"][..],
        ),
        (
            &["1", "--header"][..],
            "Git diff of the last commit",
            &["diff", "--no-color", "HEAD~1..HEAD"][..],
        ),
        (
            &["2", "--header"][..],
            "Git diff of the last 2 commits",
            &["diff", "--no-color", "HEAD~2..HEAD"][..],
        ),
        (
            &["branch", "main", "--header"][..],
            "Git diff of the current branch against main",
            &["diff", "--no-color", "main...HEAD"][..],
        ),
    ] {
        let diff = repo.git(git_args).stdout;
        let expected = expected_header(contents, "seapagan/gd", None, &diff);
        assert_stdout(&repo, args, &expected);
    }
}

#[test]
fn branch_header_uses_the_detected_base_name() {
    let repo = Repo::new("main");
    repo.write("tracked.txt", "main\n");
    repo.commit_all("main");
    repo.git(["switch", "-c", "feature"]);
    repo.write("tracked.txt", "feature\n");
    repo.commit_all("feature");
    add_origin(&repo, "git@github.com:seapagan/gd.git");
    let diff = repo.git(["diff", "--no-color", "main...HEAD"]).stdout;
    let expected = expected_header(
        "Git diff of the current branch against main",
        "seapagan/gd",
        None,
        &diff,
    );

    assert_stdout(&repo, &["branch", "--header"], &expected);
}

#[test]
fn repository_path_supports_https_and_nested_namespaces() {
    let repo = changed_repo();
    add_origin(
        &repo,
        "https://user:secret@gitlab.example.com/team/platform/gd.git",
    );
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header(
        "Git diff of unstaged changes",
        "team/platform/gd",
        None,
        &diff,
    );

    assert_stdout(&repo, &["--header"], &expected);
}

#[test]
fn current_branch_upstream_remote_precedes_origin() {
    let repo = changed_repo();
    add_origin(&repo, "https://example.com/fallback/origin.git");
    repo.git([
        "remote",
        "add",
        "upstream",
        "ssh://git@git.example.com/team/upstream.git",
    ]);
    let head = String::from_utf8(repo.git(["rev-parse", "HEAD"]).stdout).unwrap();
    repo.git(["update-ref", "refs/remotes/upstream/main", head.trim()]);
    repo.git(["branch", "--set-upstream-to", "upstream/main"]);
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header("Git diff of unstaged changes", "team/upstream", None, &diff);

    assert_stdout(&repo, &["--header"], &expected);
}

#[test]
fn origin_precedes_other_configured_remotes() {
    let repo = changed_repo();
    repo.git([
        "remote",
        "add",
        "company",
        "https://example.com/team/company.git",
    ]);
    add_origin(&repo, "https://example.com/team/origin.git");
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header("Git diff of unstaged changes", "team/origin", None, &diff);

    assert_stdout(&repo, &["--header"], &expected);
}

#[test]
fn another_usable_remote_is_used_without_origin() {
    let repo = changed_repo();
    repo.git(["remote", "add", "local", repo.path().to_str().unwrap()]);
    repo.git([
        "remote",
        "add",
        "company",
        "git@example.com:group/project.git",
    ]);
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header("Git diff of unstaged changes", "group/project", None, &diff);

    assert_stdout(&repo, &["--header"], &expected);
}

#[test]
fn repository_root_basename_is_used_without_a_usable_remote() {
    let repo = changed_repo();
    repo.git(["remote", "add", "local", repo.path().to_str().unwrap()]);
    let repository = repo.path().file_name().unwrap().to_str().unwrap();
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header("Git diff of unstaged changes", repository, None, &diff);

    assert_stdout(&repo, &["--header"], &expected);
}

#[test]
fn single_line_note_format_is_unchanged() {
    let repo = changed_repo();
    add_origin(&repo, "git@github.com:seapagan/gd.git");
    let output = repo.gd(&["--note", "Check error paths"]);
    assert_success(&output);
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let mut expected = b"# contents: Git diff of unstaged changes\n# repository: seapagan/gd\n# note: Check error paths\n\n".to_vec();
    expected.extend_from_slice(&diff);

    assert_eq!(output.stdout, expected);
    assert!(!repo.path().join("unstaged.diff").exists());
}

#[test]
fn multiline_note_line_endings_are_rendered_as_a_comment_block() {
    let repo = changed_repo();
    add_origin(&repo, "git@github.com:seapagan/gd.git");
    let diff = repo.git(["diff", "--no-color"]).stdout;

    for (note, rendered_note) in [
        (
            "Review error handling carefully\nCheck cleanup paths",
            "# note:\n#   Review error handling carefully\n#   Check cleanup paths\n",
        ),
        (
            "Review error handling carefully\r\nCheck cleanup paths",
            "# note:\n#   Review error handling carefully\n#   Check cleanup paths\n",
        ),
        (
            "Review error handling carefully\rCheck cleanup paths",
            "# note:\n#   Review error handling carefully\n#   Check cleanup paths\n",
        ),
        (
            "Review error handling carefully\n\nCheck cleanup paths",
            "# note:\n#   Review error handling carefully\n#\n#   Check cleanup paths\n",
        ),
    ] {
        let output = repo.gd(&["--note", note]);
        assert_success(&output);
        let mut expected = format!(
            "# contents: Git diff of unstaged changes\n# repository: seapagan/gd\n{rendered_note}\n"
        )
        .into_bytes();
        expected.extend_from_slice(&diff);

        assert_eq!(output.stdout, expected, "note: {note:?}");
    }
}

#[test]
fn configured_multiline_note_is_rendered_as_a_comment_block() {
    let repo = changed_repo();
    add_origin(&repo, "git@github.com:seapagan/gd.git");
    configure(
        &repo,
        "[header]\nenabled = true\nnote = \"Review errors\\nCheck cleanup\"\n",
    );
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let mut expected = b"# contents: Git diff of unstaged changes\n# repository: seapagan/gd\n# note:\n#   Review errors\n#   Check cleanup\n\n".to_vec();
    expected.extend_from_slice(&diff);

    assert_stdout(&repo, &[], &expected);
}

#[test]
fn cli_multiline_note_overrides_configured_note() {
    let repo = changed_repo();
    add_origin(&repo, "git@github.com:seapagan/gd.git");
    configure(
        &repo,
        "[header]\nenabled = true\nnote = 'Configured note'\n",
    );
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let mut expected = b"# contents: Git diff of unstaged changes\n# repository: seapagan/gd\n# note:\n#   CLI first line\n#   CLI second line\n\n".to_vec();
    expected.extend_from_slice(&diff);

    assert_stdout(
        &repo,
        &["--note", "CLI first line\nCLI second line"],
        &expected,
    );
}

#[test]
fn config_header_is_written_to_saved_file() {
    let repo = changed_repo();
    add_origin(&repo, "git@github.com:seapagan/gd.git");
    configure(&repo, "[header]\nenabled = true\n");
    let diff = repo.git(["diff", "--no-color"]).stdout;
    let expected = expected_header("Git diff of unstaged changes", "seapagan/gd", None, &diff);

    #[cfg(not(windows))]
    let output = {
        let output = repo.gd(&["--output-dir", "rendered"]);
        assert_success(&output);
        output
    };
    #[cfg(windows)]
    let messages = run_with_config(&repo, &["--output-dir", "rendered"]);
    assert_eq!(
        fs::read(repo.path().join("rendered/unstaged.diff")).unwrap(),
        expected
    );
    #[cfg(not(windows))]
    assert_eq!(output.stdout, expected);
    #[cfg(not(windows))]
    assert!(output.stderr.is_empty());
    #[cfg(windows)]
    assert!(!messages.is_empty());

    repo.gd_in(&["--header"]).unwrap();
    assert_eq!(patch(&repo, "unstaged.diff"), expected);
}
