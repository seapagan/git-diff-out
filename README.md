# git-diff-out

`git-diff-out` installs two names for the same CLI: `gd`, the normal short
command, and `git-diff-out`, a collision-safe alternative for systems where
`gd` is already taken. Both export common Git diffs to predictable diff files
or raw stdout.

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
| `gd -C`                | Copy and save unstaged tracked changes                | Clipboard and `unstaged.diff`    |
| `gd 3 \| grep TODO`    | Last three commits filtered for `TODO`               | Standard output                  |
| `gd -o review-diffs`   | Unstaged tracked changes with an output override     | `review-diffs/unstaged.diff`     |
| `gd --quiet`           | Unstaged tracked changes without a success message   | `unstaged.diff`                  |

When stdout is piped, redirected, or captured, `gd` sends the raw diff to stdout
automatically and suppresses its implicit `.diff` file. An explicit
`--output-dir`/`-o` is still honoured, producing both stdout and the requested
file. Use `--stdout`/`-p` to force stdout in an interactive terminal. The flag
cannot be combined with `--output-dir`/`-o`. In stdout mode, `gd` emits only diff
bytes on stdout; errors remain on stderr.

`--copy`/`-c` copies the diff without creating gd's normal saved file and cannot
be combined with `--output-dir`/`-o`. `--copy-save`/`-C` copies and saves when
stdout is interactive. With non-interactive stdout it copies and writes stdout;
only an explicit `--output-dir`/`-o` also saves. `-p -C` on an interactive
terminal writes stdout, copies, and saves. Every selected destination receives
the same rendered diff bytes, and failure in one destination does not undo a
successful destination.

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
```

All configuration keys are optional. If `base_branch` is omitted, `gd`
auto-detects the repository’s base/default branch.

A configured `output_dir` sets the default location used by file-output mode; it
does not force file output. CLI `--output-dir`/`-o` explicitly selects file
output for that invocation. Relative output directories are resolved from the
current working directory. CLI `--quiet` or `--verbose` overrides configured
`quiet`; `--verbose` only restores normal success messages.

`clipboard.osc52_fallback` is disabled by default. When enabled, a local session
uses OSC 52 only if its normal OS clipboard backend is unavailable. SSH sessions
always use OSC 52 regardless of this setting.

## Output safety

File output is streamed into a temporary file in the destination directory. A
successful non-empty diff atomically replaces the named diff. A successful
empty diff removes a stale destination. A failed Git command leaves any
existing destination unchanged.
