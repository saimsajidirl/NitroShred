pub mod block_protected_paths;
pub mod select_erase_method;
pub mod trim_ssd_blocks;
pub mod write_zeros_direct;
pub mod write_zeros_parallel;

#[cfg(target_os = "linux")]
pub mod write_zeros_uring;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub use select_erase_method::ShredOptions;

#[derive(Debug, Serialize, Deserialize)]
pub struct ShredResult {
    pub path: String,
    pub success: bool,
    pub error: Option<String>,
    pub mb: f64,
    pub speed_mb_s: f64,
}

pub fn shred_path(path: &Path, opts: &ShredOptions) -> anyhow::Result<Vec<ShredResult>> {
    block_protected_paths::assert_safe(path)?;

    if path.is_file() {
        Ok(vec![shred_one(path, opts)])
    } else if path.is_dir() {
        let mut files: Vec<PathBuf> = Vec::new();
        collect_files(path, &mut files)?;

        let results: Vec<ShredResult> = files.par_iter().map(|f| shred_one(f, opts)).collect();

        remove_dirs(path)?;
        Ok(results)
    } else {
        anyhow::bail!("Path {:?} does not exist or is not a regular file/directory", path);
    }
}

fn shred_one(path: &Path, opts: &ShredOptions) -> ShredResult {
    use std::time::Instant;
    let size = path.metadata().map(|m| m.len()).unwrap_or(0);
    let t0 = Instant::now();

    match select_erase_method::shred_file(path, opts) {
        Ok(()) => {
            let elapsed = t0.elapsed().as_secs_f64();
            let mb = size as f64 / 1_048_576.0;
            let speed_mb_s = if elapsed > 0.0 { mb / elapsed } else { f64::INFINITY };
            ShredResult {
                path: path.display().to_string(),
                success: true,
                error: None,
                mb,
                speed_mb_s,
            }
        }
        Err(e) => ShredResult {
            path: path.display().to_string(),
            success: false,
            error: Some(e.to_string()),
            mb: 0.0,
            speed_mb_s: 0.0,
        },
    }
}

pub fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
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

pub fn remove_dirs(dir: &Path) -> anyhow::Result<()> {
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
