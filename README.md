# flake-du

[![release](https://img.shields.io/github/v/release/kmein/flake-du?logo=github&style=flat-square)](https://github.com/kmein/flake-du/releases)
[![version](https://img.shields.io/crates/v/flake-du?logo=rust&style=flat-square)](https://crates.io/crates/flake-du)
[![deps](https://deps.rs/repo/github/kmein/flake-du/status.svg?style=flat-square&compact=true)](https://deps.rs/repo/github/kmein/flake-du)
[![license](https://img.shields.io/badge/license-MPL--2.0-blue?style=flat-square)](https://www.mozilla.org/en-US/MPL/2.0)
[![ci](https://img.shields.io/github/actions/workflow/status/kmein/flake-du/ci.yml?label=ci&logo=github-actions&style=flat-square)](https://github.com/kmein/flake-du/actions/workflows/ci.yml)

A flake.lock viewer that shows disk usage

## Usage

```bash
nix run github:kmein/flake-du
```

```
Usage: flake-du [OPTIONS] [PATH]

Arguments:
  [PATH]  Directory containing flake.lock [default: .]

Options:
      --no-self-size        Hide each input's own store size
      --no-cumulative-size  Hide cumulative subtree sizes
  -h, --help                Print help
  -V, --version             Print version
```

`flake-du` prints a recursive tree to stdout and exits, so it can be piped or redirected. Any size warning is written to stderr.

By default, the tree shows each input's own store size and its cumulative subtree size. You can hide either column with `--no-self-size` or `--no-cumulative-size`.

The tree view shows the Nix store size of each locked flake input. `follows` edges are aliases, so they always show `0 B`. Cumulative totals (marked with Σ) are deduplicated by store path, so shared inputs do not inflate subtree totals. When some sizes are unknown, ranges are shown with `≥`. Inputs are sorted by descending size.

To compute sizes, `flake-du` first resolves store paths from `nix flake archive --dry-run` and then fetches any missing locked inputs individually with `builtins.fetchTree`.

**Note:** The flake output by default wraps `flake-du` with `lix` instead of `nix` to compute sizes. This prevents the issue in CppNix where `builtins.fetchTree` re-downloads locked inputs even if they are already in the store. You can switch back to `nix` by passing `useLix = false;` when building the package.

## Changelog

See [CHANGELOG.md](CHANGELOG.md)
