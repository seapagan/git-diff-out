# git-diff-out

`git-diff-out` installs two names for the same CLI: `gd`, the normal short
command, and `git-diff-out`, a collision-safe alternative for systems where
`gd` is already taken. Both export common Git diffs to predictable diff files
or raw stdout.

## Usage

| Command                | Meaning                                              | Default Output                   |
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
| `gd 3 -p \| grep TODO` | Last three commits filtered for `TODO`               | Standard output                  |
| `gd -o review-diffs`   | Unstaged tracked changes with an output override     | `review-diffs/unstaged.diff`     |
| `gd --quiet`           | Unstaged tracked changes without a success message   | `unstaged.diff`                  |

`--stdout`/`-p` cannot be combined with `--output-dir`/`-o`. In stdout mode,
`gd` emits only diff bytes on stdout; errors remain on stderr.

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
```

All configuration keys are optional. If `base_branch` is omitted, `gd`
auto-detects the repository’s base/default branch.

A relative `output_dir` is resolved from the current working directory. CLI
`--quiet` or `--verbose` overrides configured `quiet`; `--verbose` only
restores normal success messages.

## Output safety

File output is streamed into a temporary file in the destination directory. A
successful non-empty diff atomically replaces the named diff. A successful
empty diff removes a stale destination. A failed Git command leaves any
existing destination unchanged.
