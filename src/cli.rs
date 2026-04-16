use std::path::PathBuf;

use clap::{Args, Parser};

/// A flake.lock viewer that shows disk usage, sorted by size.
/// {n}https://github.com/nix-community/flake-du
#[derive(Parser)]
#[command(version)]
pub(crate) struct Opts {
    #[command(flatten)]
    pub path_args: PathArgs,

    /// Show store paths for each input
    #[arg(long)]
    pub show_store_paths: bool,

    /// Hide cumulative subtree sizes
    #[arg(long)]
    pub no_cumulative_size: bool,
}

#[derive(Args)]
pub(crate) struct PathArgs {
    /// Directory containing flake.lock
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

