mod block_protected_paths;
mod select_erase_method;
mod trim_ssd_blocks;
mod write_zeros_direct;
mod write_zeros_parallel;

#[cfg(target_os = "linux")]
mod write_zeros_uring;

use clap::Parser;
use rayon::prelude::*;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "nitroshred",
    version = "2.0.0",
    about = "High-performance bare-metal data invalidation engine"
)]
struct Cli {
    /// Target file or directory
    path: PathBuf,

    /// Bypass read-only permissions
    #[arg(short, long)]
    force: bool,

    /// Recursively shred directory contents
    #[arg(short, long)]
    recursive: bool,

    /// Print I/O path and speed per file
    #[arg(short, long)]
    verbose: bool,

    /// Disable hardware TRIM — force zero-fill on SSD targets
    #[arg(long)]
    no_trim: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let opts = select_erase_method::ShredOptions {
        force: cli.force,
        verbose: cli.verbose,
        no_trim: cli.no_trim,
    };

    // Safety check runs unconditionally — --force does NOT bypass it
    block_protected_paths::assert_safe(&cli.path)?;

    if cli.path.is_file() {
        select_erase_method::shred_file(&cli.path, &opts)?;
    } else if cli.path.is_dir() {
        if !cli.recursive {
            anyhow::bail!(
                "{:?} is a directory — use -r/--recursive to shred directory trees",
                cli.path
            );
        }
        shred_dir(&cli.path, &opts)?;
    } else {
        anyhow::bail!("Path {:?} does not exist or is not a regular file/directory", cli.path);
    }

    Ok(())
}

fn shred_dir(dir: &std::path::Path, opts: &select_erase_method::ShredOptions) -> anyhow::Result<()> {
    let mut files: Vec<PathBuf> = Vec::new();
    collect_files(dir, &mut files)?;

    files
        .par_iter()
        .map(|f| select_erase_method::shred_file(f, opts).map_err(|e| (f.clone(), e)))
        .collect::<Vec<_>>()
        .into_iter()
        .filter_map(|r| r.err())
        .for_each(|(f, e)| eprintln!("[nitroshred] error shredding {:?}: {}", f, e));

    remove_dirs(dir)?;
    Ok(())
}

fn collect_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

fn remove_dirs(dir: &std::path::Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            remove_dirs(&path)?;
            std::fs::remove_dir(&path).ok();
        }
    }
    std::fs::remove_dir(dir).ok();
    Ok(())
}
