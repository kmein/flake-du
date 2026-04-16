use std::path::PathBuf;

use clap::{Args, Parser};

/// A flake.lock viewer that shows disk usage
/// {n}https://github.com/nix-community/flake-du
#[derive(Parser)]
#[command(version)]
pub(crate) struct Opts {
    #[command(flatten)]
    pub path_args: PathArgs,

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

