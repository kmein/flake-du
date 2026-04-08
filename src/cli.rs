use std::path::PathBuf;

use clap::{Args, Parser};

/// A flake.lock viewer that shows disk usage
/// {n}https://github.com/nix-community/flake-du
#[derive(Parser)]
#[command(version)]
pub(crate) struct Opts {
    #[command(flatten)]
    pub path_args: PathArgs,

    /// Hide each input's own store size
    #[arg(long)]
    pub no_self_size: bool,

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

#[derive(Args)]
pub(crate) struct TreeArgs {
    #[command(flatten)]
    pub path_args: PathArgs,

    /// Hide each input's own store size
    #[arg(long)]
    pub no_self_size: bool,

    /// Hide cumulative subtree sizes
    #[arg(long)]
    pub no_cumulative_size: bool,
}

impl From<Opts> for TreeArgs {
    fn from(opts: Opts) -> Self {
        Self {
            path_args: opts.path_args,
            no_self_size: opts.no_self_size,
            no_cumulative_size: opts.no_cumulative_size,
        }
    }
}
