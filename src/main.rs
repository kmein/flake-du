mod cli;
mod lock;
mod size;
mod tree_view;

use std::{
    fs::File,
    io::{self, Write},
    path::Path,
    process::Command,
};

use clap::Parser;
use eyre::{Context, Result};
use tracing_subscriber::{EnvFilter, fmt};

use tracing::{debug, warn};

use crate::{
    cli::Opts,
    lock::Lock,
    size::SizeIndex,
    tree_view::{TreeRenderOptions, render_tree_text},
};

/// Entry point for the CLI application.
fn main() -> Result<()> {
    color_eyre::install()?;
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(io::stderr)
        .init();

    let opts = Opts::parse();
    run_tree(opts)
}

/// Core logic for resolving the flake lock, gathering sizes, and rendering the tree.
fn run_tree(args: Opts) -> Result<()> {
    let Opts {
        path_args,
        show_store_paths,
        no_cumulative_size,
    } = args;
    let flake_path = path_args.path;
    let is_remote = !flake_path.exists() && (flake_path.to_string_lossy().contains(':') || flake_path.to_string_lossy().starts_with("flake:"));

    let lock_path = if is_remote {
        debug!("fetching remote flake metadata to find lockfile");
        let output = Command::new("nix")
            .arg("--quiet")
            .args(["flake", "metadata", "--json"])
            .arg(&flake_path)
            .output()
            .context("failed to run nix flake metadata")?;

        if !output.status.success() {
            eyre::bail!("failed to fetch remote flake metadata");
        }

        let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let path = metadata.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("missing path in remote flake metadata"))?;
        
        std::path::PathBuf::from(path).join("flake.lock")
    } else {
        flake_path.join("flake.lock")
    };

    debug!("reading lock file from {}", lock_path.display());
    let lock = read_lock(&lock_path)?.resolve()?;
    
    debug!("computing size index");
    let sizes = SizeIndex::load(&lock, &flake_path, &lock_path);
    
    debug!("rendering tree");
    let tree = render_tree_text(
        &lock,
        &sizes,
        TreeRenderOptions {
            show_cumulative_size: !no_cumulative_size,
            show_store_paths,
        },
    )?;

    print!("{tree}");
    io::stdout().flush()?;

    if let Some(err) = sizes.error() {
        warn!("size warning: {err}");
        eprintln!("size warning: {err}");
    }

    Ok(())
}

/// Parses the `flake.lock` JSON file into the internal `Lock` representation.
fn read_lock(lock_path: &Path) -> Result<Lock> {
    Ok(serde_json::from_reader(File::open(lock_path)?)?)
}
