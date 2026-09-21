# Unix Man Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generate a reference-quality `gd(1)` manual from the live Clap command, ship it in Unix archives, and install it in safe conventional or explicit man directories.

**Architecture:** A development-only Cargo example will own generation, with its implementation split into focused nested modules for ROFF assembly, prose, and tests. `cargo make man` will be the sole public generation entry point; the Unix installer and release workflow will consume `target/man/gd.1` without adding runtime CLI behavior.

**Tech Stack:** Rust 2024, Clap 4.6.6, `clap_mangen = "=0.3.3"`, `clap_mangen::roff`, POSIX `sh`, Cargo Make 0.37.24, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-09-21-unix-man-page-design.md`

## Global Constraints

- Preserve Rust 1.85.0 MSRV; `clap_mangen 0.3.3` declares Rust 1.85 and uses the allowed `MIT OR Apache-2.0` licence.
- Keep `clap_mangen` under `[dev-dependencies]`; do not add a runtime man command, runtime feature, or installed generator binary.
- Build syntax, names, aliases, values, defaults, conflicts, subcommands, and synopsis from `Cli::command_for("gd")`.
- Generate `target/man/gd.1`; do not commit generated ROFF.
- Keep `gd --help` unchanged and do not alter shell-completion behavior or packaging.
- Ship `gd.1` in Linux and macOS archives only; keep the Windows archive binary-only apart from `LICENSE` and `README.md`.
- Add only `GD_MAN_DIR` and `GD_SKIP_MAN=1` to the Unix installer interface; never invoke `sudo` or require `mandb`.
- Treat `/opt/gd/bin` and `/opt/gd/bin/` as the same install directory and derive `/opt/gd/share/man/man1` from both.
- Do not modify `CHANGELOG.md`, `AGENTS.md`, `CLAUDE.md`, or generated completion files.
- Run `cargo make verify` on an unchanged tree before every commit.

## Review Focus

- A trailing slash on a conventional binary directory must not change the derived man directory; Task 2 tests both forms.
- A missing archive `gd.1` must fail before either installed binary is replaced; Task 2 preserves sentinel binary contents and asserts failure.
- An arbitrary binary directory must still receive both binaries while the installer reports a successful man-page skip; Task 2 checks output and filesystem state.
- A new public Clap argument must fail the manual documentation classification check instead of inheriting weak text unnoticed; Task 1 mutates a command with a synthetic argument and asserts the exact error.
- ROFF control characters at the start of prose lines must remain escaped and renderable; Task 1 includes representative leading-dot and leading-apostrophe text in a unit test fixture and the final gate renders the real page with `groff` and `man` when available.

---

### Task 1: Development Generator and Reference Manual

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `Makefile.toml`
- Create: `examples/generate-man.rs`
- Create: `examples/man/mod.rs`
- Create: `examples/man/content.rs`
- Create: `examples/man/tests.rs`
- Include in commit: `docs/superpowers/plans/2026-09-21-unix-man-page.md`

**Interfaces:**
- Consumes: `git_diff_out::cli::Cli::command_for("gd")`, `clap::Command`, `clap_mangen::Man`, and `clap_mangen::roff`.
- Produces: `man::generate(path: &Path) -> Result<(), Box<dyn Error>>`, `man::render_manual() -> Result<Vec<u8>, Box<dyn Error>>`, and `cargo make man` writing `target/man/gd.1`.

- [ ] **Step 1: Reconfirm dependency versions before editing the manifest**

Run:

```sh
cargo info clap_mangen
cargo info cargo-make
```

Expected: `clap_mangen 0.3.3` remains current with `rust-version: 1.85` and `MIT OR Apache-2.0`; `cargo-make 0.37.24` remains current. If either current stable release changed, inspect its release notes, MSRV, Clap compatibility, and licence before changing this plan's exact pins.

- [ ] **Step 2: Add the exact development dependency**

Add this manifest section without moving runtime dependencies:

```toml
[dev-dependencies]
clap_mangen = "=0.3.3"
```

Run:

```sh
cargo update -p clap_mangen --precise 0.3.3
cargo tree -e normal,dev -i clap_mangen
cargo deny --locked check
```

Expected: `Cargo.lock` records `clap_mangen 0.3.3`; the inverse tree reaches it through a development target; cargo-deny reports advisories, bans, licences, and sources as `ok` apart from existing duplicate-version warnings.

- [ ] **Step 3: Write failing generator tests and the example shell**

Create `examples/generate-man.rs` as the thin development entry point:

```rust
use std::{env, error::Error, io, path::Path};

mod man;

fn main() -> Result<(), Box<dyn Error>> {
    let output = env::args_os().nth(1).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "usage: generate-man OUTPUT")
    })?;
    if env::args_os().nth(2).is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: generate-man OUTPUT",
        )
        .into());
    }
    man::generate(Path::new(&output))
}
```

Create `examples/man/mod.rs` with only the test module declaration:

```rust
#[cfg(test)]
mod tests;
```

Create `examples/man/tests.rs` with tests that import the not-yet-written interfaces:

```rust
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
    let mut previous = 0;
    for section in SECTIONS {
        let marker = if section.contains(' ') {
            format!(".SH \"{section}\"")
        } else {
            format!(".SH {section}")
        };
        let position = roff.find(&marker).unwrap_or_else(|| panic!("missing {marker}"));
        assert!(position >= previous, "section order: {section}");
        assert_eq!(roff.matches(&marker).count(), 1, "duplicate {section}");
        previous = position;
    }
    for syntax in [
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
    ] {
        assert!(roff.contains(syntax), "missing {syntax}");
    }
    for reference_text in [
        "An explicit BASE takes precedence",
        "With non\\-terminal stdout",
        "Every selected destination receives the header",
        "XDG_CONFIG_HOME",
        "gitrevisions",
    ] {
        assert!(roff.contains(reference_text), "missing {reference_text}");
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
```

- [ ] **Step 4: Run the example test target and confirm RED**

Run:

```sh
cargo test --locked --example generate-man
```

Expected: compilation fails because `generate`, `render_manual`, `validate_argument_docs`, and `render_test_section` do not exist.

- [ ] **Step 5: Implement command augmentation and drift validation**

In `examples/man/mod.rs`, define these exact interfaces:

```rust
use std::{collections::BTreeSet, error::Error, fs, io, path::Path};

use clap::{ArgAction, Command};
use clap_mangen::{Man, roff};
use git_diff_out::cli::Cli;

mod content;

const TERSE_ARGUMENTS: &[&str] = &["completions/shell"];

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
    manual.render_title(&mut output)?;
    manual.render_name_section(&mut output)?;
    manual.render_synopsis_section(&mut output)?;
    content::render_description(&mut output)?;
    content::render_diff_specifications(&mut output)?;
    content::render_output(&mut output)?;
    manual.render_options_section(&mut output)?;
    manual.render_subcommands_section(&mut output)?;
    content::render_subcommand_details(&command, &mut output)?;
    content::render_configuration(&mut output)?;
    content::render_environment(&mut output)?;
    content::render_examples(&mut output)?;
    content::render_exit_status(&mut output)?;
    content::render_files(&mut output)?;
    content::render_see_also(&mut output)?;
    Ok(output)
}
```

Add a `MAN_HELP: &[(&str, &str)]` table keyed only by relative Clap argument ID paths:

```rust
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
```

Review this text against these contracts before running GREEN:

- `mode_name`: accepted aliases, positive commit counts, default unstaged selection, and exclusion of untracked files;
- `base`: branch-mode-only use and explicit CLI precedence over configuration and detection;
- `stdout`: forced terminal stdout, automatic non-terminal stdout, conflict with `output_dir`, and raw bytes only when headers are disabled;
- `copy`: clipboard without implicit file output, conflict with `copy_save` and `output_dir`, and empty-diff behavior;
- `copy_save`: terminal and non-terminal destination differences and explicit output-directory behavior;
- `output_dir`: explicit file selection, current-directory resolution for relative paths, and coexistence with automatic captured stdout;
- `header`: common-payload annotation and configured-note interaction;
- `no_header`: precedence over configured headers and pipeline use;
- `note`: header implication, configured-note precedence, whitespace-only note handling, and conflict with `no_header`;
- `quiet`: suppression limited to successful file-output messages;
- `verbose`: override of configured quiet mode without adding extra diagnostics.

Implement `documented_command()` by taking `Cli::command_for("gd")`, applying `long_help` with `Command::mut_args` and recursive `Command::mut_subcommands`, then calling `validate_argument_docs`. Build relative paths as `id` at the root and `subcommand/id` below it. Do not store short or long flag spellings in `MAN_HELP`.

Implement `validate_argument_docs(&Command) -> Result<(), String>` by collecting every visible user-defined argument path recursively. Exempt `ArgAction::Help`, `HelpShort`, `HelpLong`, and `Version`. Compare that set with the disjoint union of `MAN_HELP` paths and `TERSE_ARGUMENTS`; return these errors as applicable:

```text
undocumented man-page argument: PATH
man-page documentation refers to missing argument: PATH
argument has both rich and terse man-page documentation: PATH
```

Implement `render_test_section` through `roff::Roff::text` so the test exercises the same escaping path as real prose.

- [ ] **Step 6: Write the manual sections in `examples/man/content.rs`**

Use `roff::Roff`, `roff::roman`, `roff::bold`, and `roff::italic`. Render headings with `.SH`, ordinary paragraphs with `.PP`, tagged definitions with `.TP`, and command examples with `.EX`/`.EE`. Use the exact section order from the test.

Expose these functions to `examples/man/mod.rs`:

```rust
pub(super) fn render_description(output: &mut Vec<u8>) -> io::Result<()>;
pub(super) fn render_diff_specifications(output: &mut Vec<u8>) -> io::Result<()>;
pub(super) fn render_output(output: &mut Vec<u8>) -> io::Result<()>;
pub(super) fn render_subcommand_details(
    command: &clap::Command,
    output: &mut Vec<u8>,
) -> io::Result<()>;
pub(super) fn render_configuration(output: &mut Vec<u8>) -> io::Result<()>;
pub(super) fn render_environment(output: &mut Vec<u8>) -> io::Result<()>;
pub(super) fn render_examples(output: &mut Vec<u8>) -> io::Result<()>;
pub(super) fn render_exit_status(output: &mut Vec<u8>) -> io::Result<()>;
pub(super) fn render_files(output: &mut Vec<u8>) -> io::Result<()>;
pub(super) fn render_see_also(output: &mut Vec<u8>) -> io::Result<()>;
```

Write concise final prose covering these facts:

- DESCRIPTION: one selected Git diff becomes one rendered payload; selected destinations receive those bytes; Git owns diff semantics; diff modes require a checked-out non-bare work tree.
- DIFF SPECIFICATIONS: default/`u`/`unstaged`, `s`/`staged`, `a`/`all`, `b`/`branch [BASE]`, and positive counts; spell out `git diff --no-color`, `--staged`, `HEAD` or the empty tree, `BASE...HEAD`, and `HEAD~N..HEAD`; explain base precedence and untracked-file exclusion.
- OUTPUT: automatic captured stdout, forced stdout, implicit and explicit files, clipboard routing, `copy_save` combinations, identical rendered payloads, optional headers, empty-output cleanup, atomic file replacement, and destination-failure independence.
- SUBCOMMANDS: append a `completions` subsection after `Man::render_subcommands_section`; derive the subcommand name, `SHELL` value name, and every supported shell from the cloned Clap command rather than copying them into prose.
- CONFIGURATION: every key in `Config`, `ClipboardConfig`, and `HeaderConfig`, with defaults and CLI precedence.
- ENVIRONMENT: `XDG_CONFIG_HOME`, `HOME`, `SSH_CONNECTION`, `SSH_CLIENT`, `SSH_TTY`, `WAYLAND_DISPLAY`, `XDG_SESSION_TYPE`, `DISPLAY`, and `PATH`, limited to their real effects.
- EXAMPLES: `gd s | less`, `gd b develop`, `gd -o review-diffs`, `gd -p -C`, `gd --no-header 3 | grep TODO`, and `gd --note "Review error handling"` with explanations that add behavior rather than restate syntax.
- EXIT STATUS: 0 for success/help/version, 2 for Clap usage errors, 1 for application failures; name Git, configuration, clipboard, stdout, and file failures without inventing more codes.
- FILES: `$XDG_CONFIG_HOME/git-diff-out/config.toml`, `$HOME/.config/git-diff-out/config.toml`, `$HOME/Library/Application Support/git-diff-out/config.toml`, and the mode-specific generated filenames. Obtain file names from `Mode::filename()` for `Default`, `Staged`, `All`, `Branch`, `Commits(1)`, and a representative multi-commit count instead of copying the strings into a second Rust table.
- SEE ALSO: `git(1)`, `git-diff(1)`, `git-rev-parse(1)`, and `gitrevisions(7)`.

Do not copy README paragraphs. Do not add generic introductions, summaries, repeated option text, or examples that only reproduce SYNOPSIS.

- [ ] **Step 7: Add the Cargo Make entry point**

Add this task under the packaging/development task area in `Makefile.toml`:

```toml
[tasks.man]
description = "Generate the gd(1) manual page"
command = "cargo"
args = [
  "run",
  "--quiet",
  "--locked",
  "--example",
  "generate-man",
  "--",
  "target/man/gd.1",
]
```

- [ ] **Step 8: Run focused GREEN checks**

Run:

```sh
cargo test --locked --example generate-man
cargo make man
test -s target/man/gd.1
second_page=$(mktemp)
cargo run --quiet --locked --example generate-man -- "$second_page"
cmp target/man/gd.1 "$second_page"
rm "$second_page"
```

If `/dev/stdout` replacement semantics make the last command unsuitable, generate a second temporary page and compare it with `cmp`; do not weaken the deterministic byte comparison.

Expected: all example tests pass, `target/man/gd.1` exists, and repeated generation is byte-identical.

- [ ] **Step 9: Review prose and render locally**

Run:

```sh
MANPAGER=cat PAGER=cat man -l target/man/gd.1 > target/man/gd-man.txt
groff -man -Tutf8 target/man/gd.1 > target/man/gd.txt
sed -n '1,260p' target/man/gd.txt
```

Use whichever local renderer exists; this host has both `man` and `groff`. Check option wrapping, indentation, apostrophes, hyphens, section order, and cross-references. Apply the `stop-slop` checks to every hand-written paragraph: remove filler and adverbs, use active voice, break formulaic contrasts, vary rhythm, and score at least 35/50 across directness, rhythm, trust, authenticity, and density.

- [ ] **Step 10: Run the full pre-commit gate and inspect scope**

Run:

```sh
cargo make test
cargo make verify
git status --short --branch
git diff --check
git diff
git diff --cached
```

Expected: all gates pass; existing complexity findings remain advisory; changes are limited to the plan, manifest/lockfile, Make task, and generator files.

- [ ] **Step 11: Commit the generator**

Run with elevated Git access:

```sh
git add Cargo.toml Cargo.lock Makefile.toml examples docs/superpowers/plans/2026-09-21-unix-man-page.md
git diff --cached --check
git diff --cached
git commit -s -m "feat: generate Unix man page"
```

Expected: one signed commit containing no generated `target/man/gd.1`.

### Task 2: Unix Installer Delivery

**Files:**
- Modify: `tests/installers/unix.sh`
- Modify: `install.sh`

**Interfaces:**
- Consumes: Unix release archives containing `gd`, `git-diff-out`, and `gd.1`.
- Produces: normalized `GD_INSTALL_DIR`, optional `GD_MAN_DIR`, `GD_SKIP_MAN=1`, automatic `<prefix>/share/man/man1`, and installed `gd.1` mode 0644.

- [ ] **Step 1: Extend the local archive fixture and write failing installer cases**

Add `gd.1` to the fixture archive:

```sh
printf '.TH GD 1\n.SH NAME\ngd \\- test manual\n' > "$root/archive/gd.1"
tar -czf "$root/release.tar.gz" -C "$root/archive" gd git-diff-out gd.1
```

Add isolated assertions for these cases, resetting `GD_INSTALL_DIR`, `GD_MAN_DIR`, `GD_SKIP_MAN`, `XDG_BIN_HOME`, and `HOME` before each group:

```sh
GD_INSTALL_DIR="$root/prefix/bin"
run_install
test -f "$root/prefix/share/man/man1/gd.1" || fail 'conventional man page was not installed'
test ! -x "$root/prefix/share/man/man1/gd.1" || fail 'man page is executable'

GD_INSTALL_DIR="$root/trailing/bin/"
run_install
test -f "$root/trailing/share/man/man1/gd.1" || fail 'trailing slash changed man directory derivation'

GD_INSTALL_DIR="$root/arbitrary"
run_install
test -x "$root/arbitrary/gd" || fail 'arbitrary directory did not receive gd'
grep -q 'Skipped man page' "$root/output" || fail 'arbitrary directory skip was not reported'

GD_MAN_DIR="$root/manual/man1"
run_install
test -f "$GD_MAN_DIR/gd.1" || fail 'GD_MAN_DIR was not used'

GD_SKIP_MAN=1
GD_MAN_DIR="$root/ignored/man1"
run_install
test ! -e "$GD_MAN_DIR/gd.1" || fail 'GD_SKIP_MAN did not take precedence'
```

For the missing-page case, create a second archive containing only the binaries, place sentinel contents in the install directory, run with a conventional `/bin` directory, assert failure, and assert both sentinels remain unchanged. For the filesystem failure case, make the parent of `GD_MAN_DIR` a regular file, assert failure, and assert installed binary sentinels remain unchanged.

Cover the default layout with empty `XDG_BIN_HOME`, a temporary `HOME`, and no explicit install or man directory. Assert both `$HOME/.local/bin/gd` and `$HOME/.local/share/man/man1/gd.1` exist. Preserve the existing platform, downloader, version, PATH-warning, and executable replacement cases.

- [ ] **Step 2: Run the installer harness and confirm RED**

Run:

```sh
cargo make unix-installer-test
```

Expected: failure at the first new man-page assertion because `install.sh` still installs only the binaries.

- [ ] **Step 3: Implement normalized destination selection**

Add a POSIX-shell helper that removes trailing slashes without turning `/` into an empty path:

```sh
normalize_dir() {
    value=$1
    while [ "$value" != / ] && [ "${value%/}" != "$value" ]; do
        value=${value%/}
    done
    printf '%s\n' "$value"
}
```

Normalize the selected binary directory, then apply this precedence exactly:

```sh
install_dir=$(normalize_dir "${GD_INSTALL_DIR:-${XDG_BIN_HOME:-$HOME/.local/bin}}")
man_dir=
if [ "${GD_SKIP_MAN:-}" = 1 ]; then
    printf 'Skipped man page installation because GD_SKIP_MAN=1.\n'
elif [ -n "${GD_MAN_DIR:-}" ]; then
    man_dir=$(normalize_dir "$GD_MAN_DIR")
else
    case "$install_dir" in
        */bin) man_dir=${install_dir%/bin}/share/man/man1 ;;
        *) printf 'Skipped man page installation: set GD_MAN_DIR for binary directory %s.\n' "$install_dir" ;;
    esac
fi
```

Validate `gd`, `git-diff-out`, and the selected `gd.1` before creating destination directories. Create both selected directories before replacing any installed file. Install binaries with mode 0755 and the page with mode 0644. Print one success line for the binaries and, when selected, one for the page. Keep the existing PATH warning unchanged apart from using the normalized binary path.

- [ ] **Step 4: Run focused installer checks and ShellCheck**

Run:

```sh
cargo make unix-installer-test
cargo make installers-shellcheck
```

Expected: all existing and new cases pass; ShellCheck reports no findings.

- [ ] **Step 5: Inspect installer complexity before integration work**

Run:

```sh
wc -l install.sh tests/installers/unix.sh
shellcheck install.sh tests/installers/unix.sh
git diff --check
git diff -- install.sh tests/installers/unix.sh
```

Expected: the installer retains direct POSIX control flow, no suppression comments, no privilege escalation, and no unrelated downloader or completion changes.

### Task 3: Release Archives and User Documentation

**Files:**
- Modify: `.github/workflows/release.yml`
- Modify: `README.md`
- Modify: `TODO.md`

**Interfaces:**
- Consumes: `cargo make man` from Task 1 and the `gd.1` archive contract consumed by Task 2.
- Produces: Unix archives containing `gd.1`, unchanged Windows archive contents, and user-facing installation guidance.

- [ ] **Step 1: Record the configuration-only test exception and reproduce the missing artifact**

The release workflow is configuration, so this task uses the approved plan's explicit exception to code-level TDD. Before editing, inspect the current Unix and Windows package steps and run:

```sh
cargo make man
test -s target/man/gd.1
rg -n 'Package Unix archive|Package Windows archive|gd\.1|cargo make man' .github/workflows/release.yml
```

Expected before the edit: `gd.1` exists locally but the workflow contains no `gd.1` or `cargo make man` integration.

- [ ] **Step 2: Install pinned Cargo Make only for Unix release legs**

After the release build step, add:

```yaml
      - name: Install Cargo Make
        if: matrix.runner != 'windows-latest'
        run: cargo install cargo-make --version 0.37.24 --locked

      - name: Generate man page
        if: matrix.runner != 'windows-latest'
        run: cargo make man
```

Keep the current Rust 1.98.1 workflow toolchain. Cargo Make 0.37.24 is the verified current stable release; its release notes contain maintenance and optimization changes and no incompatible invocation change for `cargo make man`.

- [ ] **Step 3: Add `gd.1` only to Unix archives**

Extend the existing Unix package step with:

```sh
cp target/man/gd.1 "$archive_root/gd.1"
tar -czf "$ARCHIVE_NAME" -C "$archive_root" gd git-diff-out gd.1 LICENSE README.md
```

Do not change the Windows `Copy-Item` or `Compress-Archive` paths.

- [ ] **Step 4: Update installation documentation and the completed task**

Edit the README installation sections with these facts:

- The Unix installer installs `gd.1` beside conventional `<prefix>/bin` layouts under `<prefix>/share/man/man1`.
- `GD_MAN_DIR` selects another `man1` directory; `GD_SKIP_MAN=1` installs binaries only; an arbitrary binary directory without either setting receives the binaries and an informational skip message.
- Linux and macOS release archives include `gd.1`; Windows archives do not.
- `cargo-binstall` installs the package binaries only, even though it consumes prebuilt release assets.
- `cargo install` installs only `gd` and `git-diff-out`.
- Every installation method still leaves shell completions to `gd completions <shell>`.

Keep this text in the existing Installation section. Do not add generator internals or repeat the Shell completions section.

Remove only this completed bullet from `TODO.md`:

```text
- Man-page generation.
```

- [ ] **Step 5: Validate workflow syntax and reproduce archive contents locally**

Run:

```sh
cargo make actions-lint
cargo make zizmor
cargo build --locked --bins
archive_root=$(mktemp -d)
trap 'rm -rf "$archive_root"' EXIT
cp target/debug/gd target/debug/git-diff-out target/man/gd.1 LICENSE README.md "$archive_root/"
tar -czf "$archive_root/unix.tar.gz" -C "$archive_root" gd git-diff-out gd.1 LICENSE README.md
tar -tzf "$archive_root/unix.tar.gz"
```

Expected Unix member set:

```text
gd
git-diff-out
gd.1
LICENSE
README.md
```

Inspect the unchanged Windows block and confirm its `Compress-Archive` list remains exactly:

```text
gd.exe
git-diff-out.exe
LICENSE
README.md
```

- [ ] **Step 6: Run the complete pre-commit verification**

Run:

```sh
cargo make man
cargo make test
cargo make unix-installer-test
cargo make installers-shellcheck
cargo make verify
git status --short --branch
git diff --check
git diff
git diff --cached
```

Expected: every command passes; the generated page stays ignored under `target/`; no completion, Windows installer, application behavior, or changelog file changed.

- [ ] **Step 7: Commit delivery integration**

Run with elevated Git access:

```sh
git add install.sh tests/installers/unix.sh .github/workflows/release.yml README.md TODO.md
git diff --cached --check
git diff --cached
git commit -s -m "feat: ship Unix man page"
```

Expected: a signed commit containing installer, release, tests, and user documentation only.

### Task 4: Final Proof, Push, and Draft Pull Request

**Files:**
- Inspect only: all branch changes relative to `main`
- Generated only: `target/man/gd.1`, `target/man/gd.txt`, temporary archive directories

**Interfaces:**
- Consumes: both implementation commits and the approved specification.
- Produces: clean pushed branch `feat/unix-man-page` and a draft pull request.

- [ ] **Step 1: Render and inspect the final page**

Run:

```sh
cargo make man
groff -man -Tutf8 target/man/gd.1 > target/man/gd.txt
sed -n '1,320p' target/man/gd.txt
MANPAGER=cat PAGER=cat man -l target/man/gd.1 > target/man/gd-man.txt
```

Expected: section-1 layout, readable wrapping, no raw ROFF leakage, no duplicated sections, accurate cross-references, and prose passing the stop-slop review.

- [ ] **Step 2: Run final repository gates on the committed tree**

Run:

```sh
cargo make test
cargo make verify
cargo make package
cargo make package-list
```

Expected: all gates pass. `cargo make package-list` includes generator sources and documentation as normal crate source files but does not include `target/man/gd.1`.

- [ ] **Step 3: Audit scope, history, signatures, and cleanliness**

Run:

```sh
git status --short --branch
git diff main...HEAD --check
git diff --stat main...HEAD
git log --show-signature --format='%H%n%s%n%(trailers:key=Signed-off-by,valueonly)' 6bee350..HEAD
for commit in $(git rev-list --reverse 6bee350..HEAD); do
    git verify-commit "$commit"
    test "$(git show -s --format='%(trailers:key=Signed-off-by,valueonly)' "$commit" | sed '/^$/d' | wc -l)" -eq 1
done
git diff main...HEAD -- Cargo.toml Cargo.lock Makefile.toml examples install.sh tests/installers/unix.sh .github/workflows/release.yml README.md TODO.md
```

Expected: only approved files plus the spec and plan changed; every implementation commit after `6bee350` has a cryptographically valid Git signature and one `Signed-off-by` trailer; the worktree is clean; no generated page, completion file, or local agent file is tracked.

- [ ] **Step 4: Push the feature branch**

Resolve live refs first:

```sh
git status -sb
git rev-parse HEAD
git rev-parse refs/heads/main
git rev-parse refs/remotes/origin/main
git push -u origin feat/unix-man-page
```

Expected: `origin/feat/unix-man-page` points to the local HEAD and `main` remains unchanged.

- [ ] **Step 5: Create and verify the draft pull request**

Use an elevated GitHub CLI call with title:

```text
feat: ship Unix man page
```

Use a body with these sections and facts:

```markdown
## Summary

- generates `gd(1)` from the live Clap command with richer reference sections and argument-ID drift checks
- installs the manual through conventional Unix prefixes or `GD_MAN_DIR`, with `GD_SKIP_MAN=1` and safe arbitrary-directory behavior
- adds `gd.1` to Linux and macOS archives while leaving Windows archives and shell completions unchanged

## Manual

The page covers diff specifications, destination selection, configuration precedence, environment-sensitive clipboard behavior, examples, exit status, files, and related Git manuals. `cargo make man` writes the deterministic artifact to `target/man/gd.1`.

## Verification

- `cargo make man`
- `cargo make test`
- `cargo make unix-installer-test`
- `cargo make installers-shellcheck`
- `cargo make actions-lint`
- `cargo make zizmor`
- `cargo make verify`
- `cargo make package`
- local `man -l` and `groff -man -Tutf8` rendering
- representative Unix and Windows archive-content inspection
```

Create with `gh pr create --draft`, then read it back with:

```sh
gh pr view --json number,title,isDraft,headRefName,baseRefName,url,body
```

Expected: draft state, base `main`, head `feat/unix-man-page`, accurate body, and no merge or release action.
