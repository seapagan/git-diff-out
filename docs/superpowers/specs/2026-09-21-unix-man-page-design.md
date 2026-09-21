# Unix Man Page Design

## Purpose

Ship a section-1 `gd` manual with Unix release archives and install it through
the existing Unix installer. The manual must use the live Clap command as its
source for syntax while adding reference material that is too detailed for
`gd --help`.

The generated `gd.1` remains a build artifact under `target/`. Git will track
the generator and prose source, not the generated ROFF.

## Scope

This change adds:

- a development-only Rust man-page generator;
- a `cargo make man` task;
- deterministic tests for the generated manual;
- `gd.1` in Linux and macOS release archives;
- Unix installer support for conventional and explicit man directories; and
- concise installation documentation.

The change does not add a runtime `gd` command, alter short help, install shell
completions, change Windows archives, or make `cargo install` install data
files.

## Generator

`examples/generate-man.rs` will contain the generator and its tests. Cargo will
compile it only as a development example, so `clap_mangen` can remain a
development dependency. The example will accept one output path and create its
parent directory. `cargo make man` will invoke it with
`target/man/gd.1`.

The generator will start with `Cli::command_for("gd")`. It may clone and
augment that `clap::Command`, but it will not recreate names, spellings,
aliases, value syntax, defaults, conflicts, possible values, subcommands, or
synopsis lines.

Man-only argument text will be attached by Clap argument ID. A validation pass
will walk the root command and its subcommands. Each user-defined argument must
have either man-only text or an explicit terse classification. Clap's generated
help and version arguments are exempt. The generator will fail when an unknown
argument appears, when a documented ID no longer exists, or when one ID appears
in both classifications. This check forces a deliberate documentation decision
for new public arguments without duplicating their flag spellings.

The generator will use `clap_mangen::Man` section renderers for NAME, SYNOPSIS,
OPTIONS, and SUBCOMMANDS. It will use `clap_mangen`'s ROFF types for the custom
sections. The title, section number, source, and manual name will use stable
values so repeated generation produces identical bytes.

## Manual Content

The `gd(1)` manual will contain these sections in this order:

1. NAME
2. SYNOPSIS
3. DESCRIPTION
4. DIFF SPECIFICATIONS
5. OUTPUT
6. OPTIONS
7. SUBCOMMANDS
8. CONFIGURATION
9. ENVIRONMENT
10. EXAMPLES
11. EXIT STATUS
12. FILES
13. SEE ALSO

DESCRIPTION will establish the operating model: `gd` selects one Git diff,
renders one payload, and sends that payload to one or more destinations. It
will state that Git supplies diff semantics and that diff commands require a
checked-out work tree.

DIFF SPECIFICATIONS will describe default and explicit unstaged mode, staged
mode, all tracked changes, branch mode with explicit or detected bases, and
positive commit counts. It will cover the empty-tree behavior before the first
commit and the exclusion of untracked files.

OUTPUT will describe terminal-sensitive stdout selection, explicit stdout,
file naming, explicit output directories, clipboard-only and copy-and-save
behavior, identical rendered bytes across selected destinations, headers,
empty diffs, and safe replacement of existing files.

OPTIONS will retain Clap's generated option syntax. Man-only descriptions will
add interaction, precedence, default, and consequence details only where those
details help users choose or combine options.

CONFIGURATION will document every accepted TOML key and its default or
precedence rule. ENVIRONMENT will cover configuration-path variables and the
session variables that influence clipboard backend selection. FILES will name
only the Linux and macOS configuration paths used by the current `directories`
integration and the generated diff files.

EXAMPLES will answer concrete workflow questions: review staged work in a
pager, compare a branch with an explicit base, save under another directory,
copy and save one payload, suppress a configured header for a pipeline, and add
a review note. EXIT STATUS will document Clap success/display exits separately
from parse errors and the application's exit code for Git, configuration,
clipboard, and file-output failures.

SEE ALSO will reference `git(1)`, `git-diff(1)`, `git-rev-parse(1)`, and
`gitrevisions(7)`.

All hand-written manual prose will receive a stop-slop review for directness,
density, active voice, varied rhythm, and removal of generic or repetitive
text. A local `man` or `groff` rendering will supplement automated checks when
the tool exists, but repository verification will not require either program.

## Unix Installer

The existing environment-variable interface will gain:

- `GD_MAN_DIR`, an explicit section-1 manual directory; and
- `GD_SKIP_MAN=1`, which suppresses man-page installation.

The installer will choose a manual destination in this order:

1. skip when `GD_SKIP_MAN=1`;
2. use non-empty `GD_MAN_DIR`;
3. when `GD_INSTALL_DIR` or its default ends in `/bin`, replace that suffix
   with `/share/man/man1`; or
4. print an informational message and install only the binaries.

The installer will not infer a manual hierarchy from an arbitrary binary
directory. It will create the chosen manual directory, install `gd.1` with mode
0644, and leave permission failures to normal filesystem handling. It will not
invoke `sudo` or require `mandb`.

Before installing any selected payload, the installer will confirm that the
archive contains both binaries and, when required, `gd.1`. This avoids updating
the binaries before discovering an incomplete archive.

## Release Archives

Each non-Windows release job will run the same generator through
`cargo make man`, copy `target/man/gd.1` into the archive root, and include it
in the tar archive beside both binaries, `LICENSE`, and `README.md`.

The Windows archive command will remain unchanged and will contain no man page.
Shell-completion generation and packaging remain outside this change.

## Documentation

README installation text will state:

- the Unix project installer installs the manual when it can derive or receives
  a safe destination;
- `GD_MAN_DIR` overrides that destination and `GD_SKIP_MAN=1` skips it;
- Unix release archives contain `gd.1`;
- `cargo install` installs binaries only; and
- shell completions still require `gd completions <shell>`.

The completed man-page item will be removed from `TODO.md`. `CHANGELOG.md` will
not change.

## Tests and Verification

Generator tests will prove that:

- generation succeeds from the real `Cli::command_for("gd")` command;
- two generations produce identical bytes;
- all required sections occur once and in the required order;
- representative modes, options, conflicts, defaults, values, and the
  `completions` subcommand appear;
- the output identifies `gd` as a section-1 page; and
- the argument-documentation classification matches the live Clap tree.

The Unix installer harness will build a local archive containing `gd.1` and
exercise:

- the default `~/.local/bin` and matching man hierarchy;
- a conventional custom `<prefix>/bin`;
- an arbitrary binary directory with an informational skip;
- `GD_MAN_DIR`;
- `GD_SKIP_MAN=1`;
- directory creation and replacement;
- a missing `gd.1` when installation requires it; and
- a safe filesystem failure path.

Release-workflow review will verify that all three Unix matrix legs execute the
conditional generation and archive step while the Windows leg retains its
existing contents. `actionlint` and Zizmor will validate the workflow.

Before each commit, `cargo make verify` must pass on the unchanged staged
scope. Final verification will also run `cargo make man`, inspect and render
`target/man/gd.1`, run `cargo make test`, exercise the installer scenarios,
inspect representative Unix and Windows archive contents, and rerun
`cargo make verify`.
