use std::{collections::BTreeSet, error::Error, fs, io, path::Path};

use clap::{ArgAction, Command};
use clap_mangen::Man;
use git_diff_out::cli::Cli;

mod content;

#[cfg(test)]
mod tests;

const TERSE_ARGUMENTS: &[&str] = &["completions/shell"];

const MAN_HELP: &[(&str, &str)] = &[
    (
        "mode_name",
        "Select unstaged (u or unstaged), staged (s or staged), all tracked \
         uncommitted changes (a or all), branch changes (b or branch), or the \
         changes from a positive number of recent commits. Omitting MODE \
         selects unstaged changes. No mode includes untracked files.",
    ),
    (
        "base",
        "Compare branch mode with BASE. This argument is valid only after b \
         or branch. An explicit BASE takes precedence over base_branch and \
         automatic base detection.",
    ),
    (
        "stdout",
        "Write the rendered payload to standard output in an interactive \
         terminal. Piped, redirected, or captured output selects standard \
         output without this flag. This flag conflicts with --output-dir. \
         The payload is a raw Git diff only when annotation headers are off.",
    ),
    (
        "copy",
        "Copy the rendered payload without creating the implicit diff file. \
         This flag conflicts with --copy-save and --output-dir. An empty diff \
         does not modify the clipboard.",
    ),
    (
        "copy_save",
        "Copy and save the rendered payload when stdout is a terminal. With \
         non-terminal stdout, write and copy the payload; add --output-dir to \
         save it as well. Combining this flag with --stdout in a terminal \
         selects stdout, clipboard, and the implicit file.",
    ),
    (
        "output_dir",
        "Write the mode-specific diff file under PATH. Relative paths start \
         at the current working directory. An explicit directory selects file \
         output and remains active when captured stdout selects stdout too.",
    ),
    (
        "header",
        "Prepend the annotation header to the rendered payload. Every selected \
         destination receives the header. A configured note appears when this \
         flag enables a header.",
    ),
    (
        "no_header",
        "Omit the annotation header for this invocation, overriding \
         configuration. Use this flag when a pipeline requires a raw Git diff.",
    ),
    (
        "note",
        "Add TEXT to the annotation header and enable that header. TEXT \
         overrides a configured note; whitespace-only text suppresses the \
         configured note while retaining the header. This option conflicts \
         with --no-header.",
    ),
    (
        "quiet",
        "Suppress successful file-output messages. Errors, Git diagnostics, \
         and raw stdout data remain unchanged.",
    ),
    (
        "verbose",
        "Restore normal file-output messages when configuration enables quiet \
         mode. This flag does not add diagnostic detail.",
    ),
];

pub fn generate(path: &Path) -> Result<(), Box<dyn Error>> {
    let page = render_manual()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, page)?;
    Ok(())
}

pub fn render_manual() -> Result<Vec<u8>, Box<dyn Error>> {
    let command = documented_command()?;
    let manual = Man::new(command.clone())
        .title("GD")
        .section("1")
        .source(concat!("git-diff-out ", env!("CARGO_PKG_VERSION")))
        .manual("General Commands Manual");
    let mut output = Vec::new();
    render_opening_sections(&manual, &mut output)?;
    render_reference_sections(&manual, &command, &mut output)?;
    Ok(output)
}

fn render_opening_sections(manual: &Man, output: &mut Vec<u8>) -> io::Result<()> {
    manual.render_title(output)?;
    manual.render_name_section(output)?;
    manual.render_synopsis_section(output)?;
    Ok(())
}

fn render_reference_sections(
    manual: &Man,
    command: &Command,
    output: &mut Vec<u8>,
) -> io::Result<()> {
    render_diff_sections(output)?;
    render_cli_sections(manual, command, output)?;
    render_system_sections(output)
}

fn render_diff_sections(output: &mut Vec<u8>) -> io::Result<()> {
    content::render_description(output)?;
    content::render_diff_specifications(output)?;
    content::render_output(output)?;
    Ok(())
}

fn render_cli_sections(manual: &Man, command: &Command, output: &mut Vec<u8>) -> io::Result<()> {
    manual.render_options_section(output)?;
    manual.render_subcommands_section(output)?;
    content::render_subcommand_details(command, output)?;
    Ok(())
}

fn render_system_sections(output: &mut Vec<u8>) -> io::Result<()> {
    content::render_configuration(output)?;
    content::render_environment(output)?;
    content::render_examples(output)?;
    content::render_exit_status(output)?;
    content::render_files(output)?;
    content::render_see_also(output)?;
    Ok(())
}

fn documented_command() -> Result<Command, String> {
    let command = Cli::command_for("gd");
    validate_argument_docs(&command)?;
    Ok(augment_command(command, ""))
}

fn augment_command(command: Command, prefix: &str) -> Command {
    let command = command.mut_args(|argument| {
        let path = argument_path(prefix, argument.get_id().as_str());
        MAN_HELP
            .iter()
            .find_map(|(id, help)| (*id == path).then_some(*help))
            .map_or(argument.clone(), |help| argument.long_help(help))
    });
    command.mut_subcommands(|subcommand| {
        let path = argument_path(prefix, subcommand.get_name());
        augment_command(subcommand, &path)
    })
}

fn argument_path(prefix: &str, id: &str) -> String {
    if prefix.is_empty() {
        id.to_owned()
    } else {
        format!("{prefix}/{id}")
    }
}

fn validate_argument_docs(command: &Command) -> Result<(), String> {
    let mut arguments = BTreeSet::new();
    collect_argument_paths(command, "", &mut arguments);
    let rich = MAN_HELP.iter().map(|(id, _)| *id).collect::<BTreeSet<_>>();
    let terse = TERSE_ARGUMENTS.iter().copied().collect::<BTreeSet<_>>();

    if let Some(path) = arguments
        .iter()
        .find(|path| !rich.contains(path.as_str()) && !terse.contains(path.as_str()))
    {
        return Err(format!("undocumented man-page argument: {path}"));
    }
    if let Some(path) = rich.union(&terse).find(|path| !arguments.contains(**path)) {
        return Err(format!(
            "man-page documentation refers to missing argument: {path}"
        ));
    }
    if let Some(path) = rich.intersection(&terse).next() {
        return Err(format!(
            "argument has both rich and terse man-page documentation: {path}"
        ));
    }
    Ok(())
}

fn collect_argument_paths(command: &Command, prefix: &str, paths: &mut BTreeSet<String>) {
    for argument in command.get_arguments() {
        if !argument.is_hide_set() && !is_generated_argument(argument.get_action()) {
            paths.insert(argument_path(prefix, argument.get_id().as_str()));
        }
    }
    for subcommand in command.get_subcommands().filter(|item| !item.is_hide_set()) {
        let path = argument_path(prefix, subcommand.get_name());
        collect_argument_paths(subcommand, &path, paths);
    }
}

fn is_generated_argument(action: &ArgAction) -> bool {
    matches!(
        action,
        ArgAction::Help | ArgAction::HelpShort | ArgAction::HelpLong | ArgAction::Version
    )
}

#[cfg(test)]
fn render_test_section(lines: &[&str]) -> String {
    let mut section = clap_mangen::roff::Roff::new();
    for line in lines {
        section.text([clap_mangen::roff::roman(*line)]);
    }
    section.render()
}
