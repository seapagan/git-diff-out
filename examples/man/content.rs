use std::io;

use clap::Command;
use clap_mangen::roff::{Inline, Roff, bold, italic, roman};
use git_diff_out::cli::Mode;

fn section(name: &str) -> Roff {
    let mut roff = Roff::new();
    roff.control("SH", [name]);
    roff
}

fn paragraph(roff: &mut Roff, text: impl Into<String>) {
    roff.control("PP", std::iter::empty::<&str>());
    roff.text([roman(text)]);
}

fn styled_paragraph(roff: &mut Roff, text: Vec<Inline>) {
    roff.control("PP", std::iter::empty::<&str>());
    roff.text(text);
}

fn definition(roff: &mut Roff, term: Vec<Inline>, text: impl Into<String>) {
    roff.control("TP", std::iter::empty::<&str>());
    roff.text(term);
    roff.text([roman(text)]);
}

fn example(roff: &mut Roff, command: &str, text: &str) {
    roff.control("EX", std::iter::empty::<&str>());
    roff.text([roman(command)]);
    roff.control("EE", std::iter::empty::<&str>());
    paragraph(roff, text);
}

pub(super) fn render_description(output: &mut Vec<u8>) -> io::Result<()> {
    let mut roff = section("DESCRIPTION");
    styled_paragraph(
        &mut roff,
        vec![
            bold("gd"),
            roman(
                " selects a common Git diff, renders one payload, and sends those bytes to each selected destination. Git defines revision and diff semantics; ",
            ),
            bold("gd"),
            roman(" chooses arguments, output routing, file names, and optional annotations."),
        ],
    );
    paragraph(
        &mut roff,
        "Diff modes require a checked-out, non-bare Git work tree. The installed git executable performs each repository query and diff operation.",
    );
    roff.to_writer(output)
}

pub(super) fn render_diff_specifications(output: &mut Vec<u8>) -> io::Result<()> {
    let mut roff = section("DIFF SPECIFICATIONS");
    definition(
        &mut roff,
        vec![bold("u, unstaged, or no MODE")],
        "Run git diff --no-color for unstaged changes to tracked files.",
    );
    definition(
        &mut roff,
        vec![bold("s, staged")],
        "Run git diff --no-color --staged for changes in the index.",
    );
    definition(
        &mut roff,
        vec![bold("a, all")],
        "Run git diff --no-color HEAD for all tracked changes in the work tree and index. Before the first commit, gd compares against Git's empty tree object.",
    );
    definition(
        &mut roff,
        vec![bold("b, branch "), italic("[BASE]")],
        "Run git diff --no-color BASE...HEAD. An explicit BASE takes precedence over the configured base_branch. Without either, gd selects the current branch's upstream remote when that remote exists. If the upstream remote is absent, gd selects origin when present, or the sole configured remote. gd examines the selected remote's HEAD and uses the matching local branch when it exists. If that local branch does not exist, gd uses the remote-tracking ref. If no remote default yields a base, gd tries local main, then local master. gd reports an error when none of these choices yields a base.",
    );
    definition(
        &mut roff,
        vec![italic("positive commit count")],
        "Run git diff --no-color HEAD~N..HEAD. A count of 1 selects the last commit; zero and non-numeric mode names are usage errors.",
    );
    paragraph(
        &mut roff,
        "These specifications exclude untracked files. Add files to the index before using staged or all mode when the diff must include them.",
    );
    roff.to_writer(output)
}

pub(super) fn render_output(output: &mut Vec<u8>) -> io::Result<()> {
    let mut roff = section("OUTPUT");
    paragraph(
        &mut roff,
        "With a terminal on standard output, gd writes a mode-specific file. The default directory is the current directory; configured output_dir or --output-dir selects another. With non-terminal stdout, including a pipe, redirection, or capture, gd writes the payload to stdout and omits the implicit file. --stdout forces stdout in a terminal. --output-dir remains active beside automatic non-terminal stdout.",
    );
    paragraph(
        &mut roff,
        "--copy selects the clipboard without an implicit file. --copy-save selects the clipboard and, in a terminal, the file. With non-terminal stdout, --copy-save selects stdout and the clipboard; combine it with --output-dir to save the file too. In a terminal, --stdout --copy-save selects all three destinations.",
    );
    paragraph(
        &mut roff,
        "gd renders the diff and optional annotation header once. Every selected destination receives the same rendered payload. The header identifies the diff and repository and may include a note; --no-header preserves a raw Git diff for downstream tools.",
    );
    paragraph(
        &mut roff,
        "An empty diff leaves stdout empty, does not change the clipboard, and removes a stale mode-specific file when file output is active. File output uses a temporary file in the destination directory and replaces the final path only after Git succeeds. When several destinations are active, gd attempts each one and reports their combined failures.",
    );
    roff.to_writer(output)
}

pub(super) fn render_subcommands(command: &Command, output: &mut Vec<u8>) -> io::Result<()> {
    render_subcommands_with_docs(
        command,
        super::RICH_SUBCOMMANDS,
        super::TERSE_SUBCOMMANDS,
        output,
    )
}

pub(super) fn render_subcommands_with_docs(
    command: &Command,
    rich_docs: &[super::RichSubcommandDoc],
    terse_docs: &[&str],
    output: &mut Vec<u8>,
) -> io::Result<()> {
    let mut roff = section("SUBCOMMANDS");
    render_subcommand_entries(command, "", rich_docs, terse_docs, &mut roff);
    roff.to_writer(output)
}

fn render_subcommand_entries(
    command: &Command,
    prefix: &str,
    rich_docs: &[super::RichSubcommandDoc],
    terse_docs: &[&str],
    roff: &mut Roff,
) {
    for subcommand in command.get_subcommands().filter(|item| !item.is_hide_set()) {
        let path = super::argument_path(prefix, subcommand.get_name());
        let display_path = path.replace('/', " ");
        if let Some(doc) = rich_docs.iter().find(|doc| doc.path == path) {
            (doc.render)(subcommand, &display_path, roff);
        } else if terse_docs.contains(&path.as_str()) {
            render_terse_subcommand(subcommand, &display_path, roff);
        } else {
            unreachable!("validated subcommand lacks documentation: {path}");
        }
        render_subcommand_entries(subcommand, &path, rich_docs, terse_docs, roff);
    }
}

fn render_terse_subcommand(subcommand: &Command, display_path: &str, roff: &mut Roff) {
    let about = subcommand
        .get_about()
        .expect("validated terse subcommand defines short help");
    definition(roff, vec![bold(display_path)], about.to_string());
}

pub(super) fn render_completions(subcommand: &Command, display_path: &str, roff: &mut Roff) {
    let shell = subcommand
        .get_arguments()
        .find(|argument| argument.get_id() == "shell")
        .expect("completions defines shell");
    let value_name = shell
        .get_value_names()
        .and_then(|names| names.first())
        .map_or("VALUE", |name| name.as_str());
    let values = shell
        .get_possible_values()
        .into_iter()
        .filter(|value| !value.is_hide_set())
        .map(|value| value.get_name().to_owned())
        .collect::<Vec<_>>()
        .join(", ");
    definition(
        roff,
        vec![bold(display_path), roman(" "), italic(value_name)],
        format!(
            "Write a completion script to stdout. Supported values: {values}. Source or install the result using the selected shell's conventions."
        ),
    );
}

pub(super) fn render_configuration(output: &mut Vec<u8>) -> io::Result<()> {
    let mut roff = section("CONFIGURATION");
    paragraph(
        &mut roff,
        "gd reads TOML from the platform configuration path when an invocation needs configuration. A missing file uses the defaults; unreadable files, invalid TOML, and unknown keys cause an error. With --no-header and stdout as the only destination, gd skips configuration unless branch mode needs it to select a base.",
    );
    definition(
        &mut roff,
        vec![bold("output_dir = "), italic("PATH")],
        "Set the file directory; the default is the current directory. --output-dir overrides it. A configured directory changes where file output goes but does not select file output when stdout is non-terminal.",
    );
    definition(
        &mut roff,
        vec![bold("quiet = "), italic("BOOLEAN")],
        "Suppress successful file messages when true. The default is false. --quiet and --verbose override this setting for one invocation.",
    );
    definition(
        &mut roff,
        vec![bold("base_branch = "), italic("STRING")],
        "Set the branch-mode base. The default is unset. A command-line BASE overrides this value; automatic detection runs only when both are absent.",
    );
    definition(
        &mut roff,
        vec![bold("clipboard.osc52_fallback = "), italic("BOOLEAN")],
        "Allow OSC 52 when no local provider exists or a selected local provider fails. The default is false. SSH sessions select OSC 52 without this setting.",
    );
    definition(
        &mut roff,
        vec![bold("header.enabled = "), italic("BOOLEAN")],
        "Enable annotation headers by default. The default is false. --header and --note enable a header; --no-header takes precedence.",
    );
    definition(
        &mut roff,
        vec![bold("header.note = "), italic("STRING")],
        "Provide note text when a header is active. The default is unset. --note overrides it; whitespace-only --note text suppresses the configured note while retaining the header.",
    );
    roff.to_writer(output)
}

pub(super) fn render_environment(output: &mut Vec<u8>) -> io::Result<()> {
    let mut roff = section("ENVIRONMENT");
    definition(
        &mut roff,
        vec![bold("XDG_CONFIG_HOME, HOME")],
        "Select the configuration directory on Linux. gd uses $XDG_CONFIG_HOME/git-diff-out/config.toml when XDG_CONFIG_HOME is an absolute path. When it is unset, empty, or relative, gd uses $HOME/.config/git-diff-out/config.toml. macOS uses HOME for the Application Support path.",
    );
    definition(
        &mut roff,
        vec![bold("SSH_CONNECTION, SSH_CLIENT, SSH_TTY")],
        "A non-empty value marks an SSH session and selects OSC 52 clipboard output.",
    );
    definition(
        &mut roff,
        vec![bold("WAYLAND_DISPLAY, XDG_SESSION_TYPE, DISPLAY")],
        "Identify a Wayland or X11 session for Linux clipboard-provider selection. Wayland has precedence when both displays are present.",
    );
    definition(
        &mut roff,
        vec![bold("PATH")],
        "Supplies the executable search path for wl-copy, xclip, xsel, or pbcopy. Git uses the process executable search rules as well.",
    );
    roff.to_writer(output)
}

pub(super) fn render_examples(output: &mut Vec<u8>) -> io::Result<()> {
    let mut roff = section("EXAMPLES");
    example(
        &mut roff,
        "gd s | less",
        "Review the staged diff in a pager. The pipe selects stdout, so gd does not create staged.diff.",
    );
    example(
        &mut roff,
        "gd b develop",
        "Compare the current branch with develop using three-dot Git diff semantics, bypassing configured and detected bases.",
    );
    example(
        &mut roff,
        "gd -o review-diffs",
        "Create review-diffs when needed and write review-diffs/unstaged.diff.",
    );
    example(
        &mut roff,
        "gd -p -C",
        "In a terminal, send one rendered payload to stdout and the clipboard, then save unstaged.diff under the selected output directory.",
    );
    example(
        &mut roff,
        "gd --no-header 3 | grep TODO",
        "Search the last three commits as a raw Git diff, even when configuration enables headers.",
    );
    example(
        &mut roff,
        "gd --note \"Review error handling\"",
        "Enable the annotation header, replace any configured note, and save the annotated unstaged diff.",
    );
    roff.to_writer(output)
}

pub(super) fn render_exit_status(output: &mut Vec<u8>) -> io::Result<()> {
    let mut roff = section("EXIT STATUS");
    definition(
        &mut roff,
        vec![bold("0")],
        "The requested operation succeeded, or gd displayed help or version information.",
    );
    definition(
        &mut roff,
        vec![bold("1")],
        "An application operation failed, including Git execution, configuration loading, clipboard delivery, stdout writing, or file output.",
    );
    definition(
        &mut roff,
        vec![bold("2")],
        "Clap rejected command-line syntax or a value before the application ran.",
    );
    roff.to_writer(output)
}

pub(super) fn render_files(output: &mut Vec<u8>) -> io::Result<()> {
    let names = [
        Mode::Default,
        Mode::Staged,
        Mode::All,
        Mode::Branch(None),
        Mode::Commits(1),
        Mode::Commits(3),
    ]
    .map(|mode| mode.filename())
    .join(", ");
    let mut roff = section("FILES");
    definition(
        &mut roff,
        vec![bold("$XDG_CONFIG_HOME/git-diff-out/config.toml")],
        "Linux configuration path when XDG_CONFIG_HOME is an absolute path.",
    );
    definition(
        &mut roff,
        vec![bold("$HOME/.config/git-diff-out/config.toml")],
        "Linux configuration path when XDG_CONFIG_HOME is unset, empty, or relative.",
    );
    definition(
        &mut roff,
        vec![bold(
            "$HOME/Library/Application Support/git-diff-out/config.toml",
        )],
        "macOS configuration path.",
    );
    definition(
        &mut roff,
        vec![italic("mode-specific diff file")],
        format!(
            "File output uses {names}; other positive commit counts follow the last-N-commits.diff form."
        ),
    );
    roff.to_writer(output)
}

pub(super) fn render_see_also(output: &mut Vec<u8>) -> io::Result<()> {
    let mut roff = section("SEE ALSO");
    roff.text([
        bold("git"),
        roman("(1), "),
        bold("git-diff"),
        roman("(1), "),
        bold("git-rev-parse"),
        roman("(1), "),
        bold("gitrevisions"),
        roman("(7)"),
    ]);
    roff.to_writer(output)
}
