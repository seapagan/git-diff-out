# git-diff-out

`git-diff-out` (`gd`) is a small command-line tool for exporting common Git
diffs in a predictable form. Short commands cover unstaged changes, staged
changes, all uncommitted tracked changes, branch diffs, and recent commit
diffs. Send the result to a named `.diff` file, stdout, or the clipboard.

This makes it easier to hand diffs to reviewers, LLMs, scripts, pipelines, and
other tools without rebuilding `git diff` commands or managing output files by
hand.

The package installs two names for the same CLI: `gd`, the normal short
command, and `git-diff-out`, a collision-safe alternative when `gd` is already
taken.

## Installation

### Install script (recommended)

Linux and macOS:

```sh
curl -fsSL https://raw.githubusercontent.com/seapagan/git-diff-out/main/install.sh | sh
# or
wget -qO- https://raw.githubusercontent.com/seapagan/git-diff-out/main/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/seapagan/git-diff-out/main/install.ps1 | iex
```

The installers require no Rust toolchain. They create the install directory and
install both `gd` and `git-diff-out` (with `.exe` on Windows). Linux and macOS
use `$XDG_BIN_HOME` when set, otherwise `~/.local/bin`; Windows uses
`%USERPROFILE%\.local\bin`. Set `GD_INSTALL_DIR` to use another directory or
`GD_VERSION` to install an exact release tag. The installers do not modify
`PATH`; they warn if the directory is not already on it, leaving PATH setup to
you.

### cargo-binstall

```bash
cargo binstall git-diff-out
```

cargo-binstall prefers the prebuilt release binaries, avoiding a local build on
supported platforms. It installs both executable names:

```text
gd
git-diff-out
```

### cargo install

```bash
cargo install git-diff-out
```

This builds from source and requires Rust 1.85.0 or newer. It installs both
package binaries:

```text
gd
git-diff-out
```

### Prebuilt GitHub releases

The [GitHub Releases](https://github.com/seapagan/git-diff-out/releases) page
provides archives for:

- Linux x86_64
- Windows x86_64
- macOS Intel x86_64
- macOS Apple Silicon aarch64

Each archive contains both executable names for its platform, plus `LICENSE`
and `README.md`. Extract the archive and place the executable or executables
you want in a directory on your `PATH`.

### Build from source

```bash
git clone https://github.com/seapagan/git-diff-out.git
cd git-diff-out
cargo build --release --locked
```

The resulting binaries are:

```text
target/release/gd
target/release/git-diff-out
```

Windows adds the `.exe` suffix to both paths.

## Usage

`gd` must be run inside a checked-out Git repository; bare repositories are not
supported. Linked worktrees created with `git worktree` are supported.

| Command                | Meaning                                              | Output                           |
| ---------------------- | ---------------------------------------------------- | -------------------------------- |
| `gd`                   | Unstaged tracked changes                             | `unstaged.diff`                  |
| `gd u` / `gd unstaged` | Unstaged tracked changes                             | `unstaged.diff`                  |
| `gd s` / `gd staged`   | Staged tracked changes                               | `staged.diff`                    |
| `gd a` / `gd all`      | All uncommitted tracked changes                      | `uncommitted.diff`               |
| `gd b` / `gd branch`   | Current-branch changes relative to the detected base | `branch.diff`                    |
| `gd b develop`         | Current-branch changes relative to `develop`         | `branch.diff`                    |
| `gd 1`                 | Changes introduced by the last commit                | `last-commit.diff`               |
| `gd 3`                 | Changes introduced by the last three commits         | `last-3-commits.diff`            |
| `gd s --stdout`        | Staged tracked changes written to stdout             | Standard output                  |
| `gd -c`                | Copy unstaged tracked changes                         | Clipboard                        |
| `gd -C`                | Copy and save unstaged changes (interactive TTY)      | Clipboard and `unstaged.diff`    |
| `gd -H`                | Add an annotation header                              | Selected destination(s)          |
| `gd -N`                | Suppress a configured annotation header               | Selected destination(s)          |
| `gd --note "Review"`  | Add a header with a note                              | Selected destination(s)          |
| `gd 3 \| grep TODO`    | Last three commits filtered for `TODO`               | Standard output                  |
| `gd -o review-diffs`   | Unstaged tracked changes with an output override     | `review-diffs/unstaged.diff`     |
| `gd --quiet`           | Unstaged tracked changes without a success message   | `unstaged.diff`                  |

When stdout is piped, redirected, or captured, `gd` sends the rendered payload
to stdout automatically and suppresses its implicit `.diff` file. An explicit
`--output-dir`/`-o` is still honoured, producing both stdout and the requested
file. Use `--stdout`/`-p` to force stdout in an interactive terminal. The flag
cannot be combined with `--output-dir`/`-o`. In stdout mode, `gd` emits only
rendered payload bytes on stdout; errors remain on stderr. With annotation
headers disabled, as they are by default, that payload is the raw Git diff.

`--copy`/`-c` copies the diff without creating gd's normal saved file and cannot
be combined with `--output-dir`/`-o`. `--copy-save`/`-C` copies and saves when
stdout is interactive. With non-interactive stdout it copies and writes stdout;
only an explicit `--output-dir`/`-o` also saves. `-p -C` on an interactive
terminal writes stdout, copies, and saves. Every selected destination receives
the same rendered diff bytes, and failure in one destination does not undo a
successful destination.

Annotation headers are disabled by default. `--header`/`-H` enables the header,
while `--no-header`/`-N` disables it for one invocation even when configuration
enables it. `--note TEXT` adds note text and implies `--header`; an explicit CLI
note overrides a configured note. `--note` conflicts with `--no-header`, and
`--header` conflicts with `--no-header`.

An enabled header is part of the common rendered payload, so stdout, clipboard,
and saved files receive identical annotated bytes. Piping does not remove it;
use `--no-header` when a downstream tool requires a raw Git diff.

## Clipboard support

SSH sessions use OSC 52 automatically, writing the control sequence directly to
the controlling terminal so stdout remains raw diff data. OSC 52 payloads are
limited to 74,991 diff bytes, producing a complete Base64 control sequence under
the established conservative 100,000-byte ceiling; larger payloads fail without
truncation or partial clipboard output. The local terminal and any multiplexer
must allow OSC 52 clipboard writes. Ghostty and current Zellij support this write
path.

Local Linux sessions prefer `wl-copy` on Wayland, then `xclip` or `xsel` with the
X11 `CLIPBOARD` selection when an X display is available. Install `wl-clipboard`
for Wayland or `xclip`/`xsel` for X11 if no provider is found. macOS uses
`pbcopy`. Windows uses the native Unicode clipboard API. These system providers
retain clipboard ownership after `gd` exits where the platform requires it.

`gd a`/`gd all` compares against `HEAD` in a normal repository and against
Git's empty tree before the first commit. Genuinely untracked files are still
excluded.

`gd` delegates diff semantics to the installed `git` executable.

Generated diffs are commonly written inside a repository working tree.
Consider adding this pattern to that repository's `.gitignore` or to your
global Git ignore configuration:

```gitignore
*.diff
```

`gd` does not modify repositories or ignore files automatically.

## Branch bases

Branch mode compares the current branch with its base using Git's three-dot
diff (`BASE...HEAD`), showing changes introduced on the current branch since
the branches diverged.

`gd branch` uses an explicit CLI base first, then configured `base_branch`.
Otherwise it asks Git for the current branch's upstream remote, falling back
to `origin` or the sole remote, and inspects that remote's symbolic default
HEAD. It prefers the equivalent local branch when present, then the
remote-tracking ref. Local `main` and `master` are final fallbacks. If none
resolves, specify `gd branch <BASE>` or configure `base_branch`.

## Configuration

The optional `config.toml` is read from the platform's normal per-user
configuration directory under `git-diff-out`:

```toml
output_dir = "."
quiet = false
base_branch = "develop"

[clipboard]
osc52_fallback = false

[header]
enabled = true
note = "Review error handling carefully"
```

All configuration keys are optional. If `base_branch` is omitted, `gd`
auto-detects the repository’s base/default branch.

A configured `output_dir` sets the default location used by file-output mode; it
does not force file output. CLI `--output-dir`/`-o` explicitly selects file
output for that invocation. Relative output directories are resolved from the
current working directory. CLI `--quiet` or `--verbose` overrides configured
`quiet`; `--verbose` only restores normal success messages.

`clipboard.osc52_fallback` is disabled by default. When enabled, a local session
uses OSC 52 if its normal OS clipboard backend is unavailable or fails at runtime.
SSH sessions always use OSC 52 regardless of this setting.

`header.enabled` defaults to `false`. CLI `--header` or `--no-header` overrides
that setting. A configured `header.note` appears only when the header is enabled;
`--note` overrides it and enables the header for that invocation.

## Output safety

File output is streamed into a temporary file in the destination directory. A
successful non-empty diff atomically replaces the named diff. A successful
empty diff removes a stale destination. A failed Git command leaves any
existing destination unchanged.

## Development checks

Python is not required for ordinary Rust development on `gd`. `cargo make
verify` includes optional Python support tasks, which skip when their tools are
unavailable: `python-format` and `python-lint` require Ruff, `python-type`
requires mypy, and `python-test` requires Python 3.10+. The `complexity` task
requires Python 3.10+ and Lizard and skips when either is unavailable. If
installed tooling runs and reports an error, verification fails.

For local parity with Codacy's last documented and verified Lizard version,
install Python 3.10+ and Lizard 1.23.0. Direct use of
`python3 scripts/check_complexity.py` on Unix-like systems, or
`python scripts/check_complexity.py` on Windows, requires Python 3.10+. Ruff
and mypy provide additional optional formatting, linting, and strict type
checking for the support script. The Lizard checker analyzes non-ignored Rust
and Python source files. Lizard uses a strict 1.23.0 pin for deterministic
local behavior and known parser/output compatibility; Ruff and mypy are not
currently pinned.

For example, `uv tool` can install the optional tools in isolation:

```bash
uv tool install 'lizard==1.23.0'
uv tool install ruff
uv tool install mypy
```

`uv` is only one convenient installation method and is not required by the
project. When Python 3.10+ and Lizard are available, `cargo make complexity`
treats checker, parser, configuration, and tool-version errors as failures,
including an installed Lizard version other than 1.23.0. Complexity threshold
findings remain advisory and do not fail verification.
