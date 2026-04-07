# nix-melt

[![release](https://img.shields.io/github/v/release/nix-community/nix-melt?logo=github&style=flat-square)](https://github.com/nix-community/nix-melt/releases)
[![version](https://img.shields.io/crates/v/nix-melt?logo=rust&style=flat-square)](https://crates.io/crates/nix-melt)
[![deps](https://deps.rs/repo/github/nix-community/nix-melt/status.svg?style=flat-square&compact=true)](https://deps.rs/repo/github/nix-community/nix-melt)
[![license](https://img.shields.io/badge/license-MPL--2.0-blue?style=flat-square)](https://www.mozilla.org/en-US/MPL/2.0)
[![ci](https://img.shields.io/github/actions/workflow/status/nix-community/nix-melt/ci.yml?label=ci&logo=github-actions&style=flat-square)](https://github.com/nix-community/nix-melt/actions/workflows/ci.yml)

A ranger-like flake.lock viewer

![](https://user-images.githubusercontent.com/40620903/234416489-75f991a9-b6f0-490a-8b07-12297fe07bba.png)

## Usage

```bash
nix run github:nix-community/nix-melt
```

```
Usage: nix-melt [OPTIONS] [PATH] [COMMAND]

Commands:
  pane  Open the pane view
  tree  Print the tree view to stdout
  help  Print this message or the help of the given subcommand(s)

Options:
  -t, --time-format <TIME_FORMAT>  Format to display timestamps
                                   [default: "[year]-[month]-[day] [hour]:[minute] [offset_hour sign:mandatory]:[offset_minute]"]
  -h, --help                       Print help
  -V, --version                    Print version
```

`nix-melt [PATH]` still opens the pane view by default. The tree view is available directly as a subcommand, e.g. `nix-melt tree .`.

`nix-melt tree` prints a recursive tree to stdout and exits, so it can be piped or redirected without any terminal control sequences. Any size warning is written to stderr.

By default, the tree shows each input's own store size and its cumulative subtree size. You can hide either column with `--no-self-size` or `--no-cumulative-size`.

The tree view shows the Nix store size of each locked flake input. `follows` edges are aliases, so they always show `0 B`. Cumulative totals are deduplicated by store path, so shared inputs do not inflate subtree totals. To compute those sizes, `nix-melt` first resolves store paths from `nix flake archive --dry-run` and then fetches any missing locked inputs individually with `builtins.fetchTree`.

## Changelog

See [CHANGELOG.md](CHANGELOG.md)
