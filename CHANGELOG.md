# Changelog

## Unreleased

### Breaking Changes

- **Project renamed from nix-melt to flake-du**
  - Binary name changed: `nix-melt` → `flake-du`
  - Crates.io package: `nix-melt` → `flake-du`
  - Repository: `nix-community/nix-melt` → `nix-community/flake-du`
  - The name better reflects the tool's focus on disk usage analysis

### Features

- Remove interactive TUI pane view, simplify to tree-only output
- Add Σ symbol for cumulative totals
- Show size ranges with ≥ when some inputs are unknown
- Sort inputs by descending size
- Simplify path handling: accept directories only (default: ".")

## v0.1.3

### Fixes

- Fix build with Rust 1.80

## v0.1.2

### Fixes

- Make `lastModified` optional ([#1](https://github.com/nix-community/nix-melt/issues/2))

## v0.1.1 - 2023-04-26

### Features

- Automatically generate man pages and completions

### Fixes

- Accept non-string input attributes ([#1](https://github.com/nix-community/nix-melt/issues/1))

## v0.1.0 - 2023-04-25

First release
