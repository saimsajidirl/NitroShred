use clap::Parser;
use nitroshred_core::{ShredOptions, ShredResult, shred_path};
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

    /// Recursively shred directory contents
    #[arg(short, long)]
    recursive: bool,

    /// Print I/O path and speed per file
    #[arg(short, long)]
    verbose: bool,

    /// Full wipe: shred files then overwrite all free space + volume TRIM (drive/volume roots)
    #[arg(long)]
    full: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.path.is_dir() && !cli.recursive {
        anyhow::bail!(
            "{:?} is a directory — use -r/--recursive to shred directory trees",
            cli.path
        );
    }

    let opts = ShredOptions {
        verbose: cli.verbose,
        wipe_free_space: cli.full,
        full_drive: cli.full && cli.path.is_dir(),
    };

    let results = shred_path(&cli.path, &opts)?;

    if cli.verbose {
        for r in &results {
            if r.success {
                eprintln!(
                    "[nitroshred] {:?}  {:.1} MB  {:.0} MB/s",
                    r.path, r.mb, r.speed_mb_s
                );
            } else {
                eprintln!(
                    "[nitroshred] error: {:?}  {}",
                    r.path,
                    r.error.as_deref().unwrap_or("unknown")
                );
            }
        }
    }

    let failed: Vec<&ShredResult> = results.iter().filter(|r| !r.success).collect();
    if !failed.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}
