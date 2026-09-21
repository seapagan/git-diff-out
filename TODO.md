# TODO

## 0.2

- Add `gd config` commands:
  - show effective configuration;
  - show configuration path;
  - set values;
  - unset/reset values.

## Future / evaluate

- Once gd's clipboard API and cross-platform behavior are proven stable,
  consider extracting the CLI-oriented backend routing and OSC 52 handling into
  a standalone Rust crate for other short-lived command-line tools.
- Investigate optional untracked-file support without compromising the thin-wrapper design.
- Improve base-branch heuristics only if real repositories expose shortcomings.
