use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// A ranger-like flake.lock viewer
/// {n}https://github.com/nix-community/nix-melt
#[derive(Parser)]
#[command(version)]
pub(crate) struct Opts {
    #[command(subcommand)]
    pub command: Option<ViewCommand>,

    #[command(flatten)]
    pub args: PaneArgs,
}

#[derive(Args)]
pub(crate) struct PathArgs {
    /// Path to the flake.lock or the directory containing flake.lock
    #[arg(default_value = "flake.lock")]
    pub path: PathBuf,
}

#[derive(Args)]
pub(crate) struct PaneArgs {
    #[command(flatten)]
    pub path_args: PathArgs,

    /// Format to display timestamps
    ///
    /// See https://time-rs.github.io/book/api/format-description.html for the syntax
    #[arg(
        short,
        long,
        default_value = "[year]-[month]-[day] [hour]:[minute] [offset_hour sign:mandatory]:[offset_minute]"
    )]
    pub time_format: String,
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

#[derive(Subcommand)]
pub(crate) enum ViewCommand {
    /// Open the pane view
    Pane(PaneArgs),
    /// Print the tree view to stdout
    Tree(TreeArgs),
}
