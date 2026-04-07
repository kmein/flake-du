mod cli;
mod error;
mod lock;
mod pane;
mod size;
mod state;
mod tree_view;

use std::{
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
};

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use eyre::Result;
use time::format_description;

use crate::{
    cli::{Opts, PaneArgs, TreeArgs, ViewCommand},
    lock::Lock,
    size::SizeIndex,
    state::State,
    tree_view::{TreeRenderOptions, render_tree_text},
};

fn main() -> Result<()> {
    color_eyre::install()?;
    let opts = Opts::parse();
    match opts.command {
        Some(ViewCommand::Tree(args)) => run_tree(args),
        Some(ViewCommand::Pane(args)) => run_tui(args),
        None => run_tui(opts.args),
    }
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

fn run_tui(args: PaneArgs) -> Result<()> {
    let PaneArgs {
        path_args,
        time_format,
    } = args;
    let (lock_path, _) = resolve_paths(path_args.path);
    let mut state = State::new(
        read_lock(&lock_path)?,
        format_description::parse_borrowed::<2>(&time_format)?,
    )?;
    state.render()?;

    while let Ok(ev) = event::read() {
        let Event::Key(KeyEvent {
            code, modifiers, ..
        }) = ev
        else {
            continue;
        };

        match code {
            KeyCode::Char('q') => break,
            KeyCode::Char('c') if modifiers == KeyModifiers::CONTROL => break,
            KeyCode::Char('h') | KeyCode::Left => state.left()?,
            KeyCode::Char('j') | KeyCode::Down => state.down()?,
            KeyCode::Char('k') | KeyCode::Up => state.up()?,
            KeyCode::Char('l') | KeyCode::Right => state.right()?,
            _ => {}
        }
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
