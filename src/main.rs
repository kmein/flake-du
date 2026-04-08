mod cli;
mod lock;
mod size;
mod tree_view;

use std::{
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
};

use clap::Parser;
use eyre::Result;

use crate::{
    cli::{Opts, TreeArgs},
    lock::Lock,
    size::SizeIndex,
    tree_view::{TreeRenderOptions, render_tree_text},
};

fn main() -> Result<()> {
    color_eyre::install()?;
    let opts = Opts::parse();
    run_tree(opts.into())
}

fn run_tree(args: TreeArgs) -> Result<()> {
    let TreeArgs {
        path_args,
        no_self_size,
        no_cumulative_size,
    } = args;
    let path = path_args.path;
    let (lock_path, flake_path) = resolve_paths(path);
    let lock = read_lock(&lock_path)?.resolve()?;
    let sizes = SizeIndex::load(&lock, &flake_path, &lock_path);
    let tree = render_tree_text(
        &lock,
        &sizes,
        TreeRenderOptions {
            show_self_size: !no_self_size,
            show_cumulative_size: !no_cumulative_size,
        },
    )?;

    print!("{tree}");
    io::stdout().flush()?;

    if let Some(err) = sizes.error() {
        eprintln!("size warning: {err}");
    }

    Ok(())
}

fn resolve_paths(path: PathBuf) -> (PathBuf, PathBuf) {
    let lock_path = if path.is_dir() {
        path.join("flake.lock")
    } else {
        path.clone()
    };
    let flake_path = if path.is_dir() {
        path
    } else {
        lock_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    };

    (lock_path, flake_path)
}

fn read_lock(lock_path: &Path) -> Result<Lock> {
    Ok(serde_json::from_reader(File::open(lock_path)?)?)
}
