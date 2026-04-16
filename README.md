# flake-du

[![release](https://img.shields.io/github/v/release/kmein/flake-du?logo=github&style=flat-square)](https://github.com/kmein/flake-du/releases)
[![version](https://img.shields.io/crates/v/flake-du?logo=rust&style=flat-square)](https://crates.io/crates/flake-du)
[![deps](https://deps.rs/repo/github/kmein/flake-du/status.svg?style=flat-square&compact=true)](https://deps.rs/repo/github/kmein/flake-du)
[![license](https://img.shields.io/badge/license-MPL--2.0-blue?style=flat-square)](https://www.mozilla.org/en-US/MPL/2.0)
[![ci](https://img.shields.io/github/actions/workflow/status/kmein/flake-du/ci.yml?label=ci&logo=github-actions&style=flat-square)](https://github.com/kmein/flake-du/actions/workflows/ci.yml)

A flake.lock viewer that shows disk usage, sorted by size. Designed to be used in conjunction with tools like [`flake-edit follow`](https://github.com/a-kenji/flake-edit?tab=readme-ov-file#-flake-edit-follow) to help deduplicate and optimize your flake inputs. Originally based on [nix-melt](https://github.com/nix-community/nix-melt).

## Usage

```bash
nix run github:kmein/flake-du -- github:kmein/niveum
```

```
inputs [Σ 647.8 MiB]
├─ nixpkgs-unstable [188.2 MiB]
├─ nixpkgs [185.5 MiB]
├─ nixpkgs-old [111.4 MiB]
├─ stylix [2.8 MiB, Σ 58.4 MiB]
│  ├─ gnome-shell [17.7 MiB]
│  ├─ firefox-gnome-theme [14.2 MiB]
│  ├─ base16-vim [12.5 MiB]
│  ├─ tinted-zed [3.1 MiB]
│  ├─ base16 [2.8 MiB, Σ 2.8 MiB]
│  │  └─ fromYaml [7.5 KiB]
│  ├─ tinted-tmux [1.4 MiB]
│  ├─ base16-helix [1.1 MiB]
│  ├─ base16-fish [979.2 KiB]
│  ├─ tinted-kitty [845.7 KiB]
│  ├─ tinted-schemes [377.2 KiB]
│  ├─ tinted-foot [285.1 KiB]
│  ├─ nur: nur_2 [255.6 KiB]
│  │  ├─ flake-parts -> stylix/flake-parts [0 B]
│  │  └─ nixpkgs -> stylix/nixpkgs [0 B]
│  ├─ flake-parts: flake-parts_3 [118.2 KiB]
│  │  └─ nixpkgs-lib -> stylix/nixpkgs [0 B]
│  ├─ systems: systems_2 [2.4 KiB]
│  └─ nixpkgs -> nixpkgs [0 B]
├─ scripts [29.3 MiB]
│  ├─ fenix -> fenix [0 B]
│  ├─ naersk -> naersk [0 B]
│  └─ nixpkgs -> nixpkgs [0 B]
...
```

```
Usage: flake-du [OPTIONS] [PATH]

Arguments:
  [PATH]  Directory containing flake.lock [default: .]

Options:
      --no-cumulative-size  Hide cumulative subtree sizes
  -h, --help                Print help
  -V, --version             Print version
```

`flake-du` prints a recursive tree to stdout and exits, so it can be piped or redirected. Any size warning is written to stderr.

By default, the tree shows each input's own store size and its cumulative subtree size. You can hide the cumulative total column with `--no-cumulative-size`.

The tree view shows the Nix store size of each locked flake input. `follows` edges are aliases, so they always show `0 B`. Cumulative totals (marked with Σ) are deduplicated by store path, so shared inputs do not inflate subtree totals. When some sizes are unknown, ranges are shown with `≥`. Inputs are sorted by descending size.

To compute sizes, `flake-du` first resolves store paths from `nix flake archive --dry-run` and then fetches any missing locked inputs individually with `builtins.fetchTree`.

**Note:** The flake output by default wraps `flake-du` with `lix` instead of `nix` to compute sizes. This prevents the issue in CppNix where `builtins.fetchTree` re-downloads locked inputs even if they are already in the store. You can switch back to `nix` by passing `useLix = false;` when building the package.


