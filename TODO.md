# TODO

## 0.2

- Add `gd config` commands:
  - show effective configuration;
  - show configuration path;
  - set values;
  - unset/reset values.

## Future / evaluate

- Shell completions for Bash, Zsh, Fish and PowerShell.
- Man-page generation.
- Investigate optional untracked-file support without compromising the thin-wrapper design.
- Improve base-branch heuristics only if real repositories expose shortcomings.
- Add full GitHub Actions CI/release automation after the utility has been proven:
  - Linux;
  - macOS;
  - Windows;
  - formatting;
  - Clippy;
  - tests;
  - MSRV;
  - target/cross-compilation checks including Windows MSVC/cargo-xwin where useful;
  - dependency/security checks;
  - hardened Actions/Zizmor;
  - release binaries.
