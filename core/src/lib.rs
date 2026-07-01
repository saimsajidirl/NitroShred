pub mod block_protected_paths;
pub mod select_erase_method;
pub mod trim_ssd_blocks;
pub mod volume_trim;
pub mod wipe_free_space;
pub mod write_zeros_direct;
pub mod write_zeros_parallel;

#[cfg(target_os = "linux")]
pub mod write_zeros_uring;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub use select_erase_method::ShredOptions;

#[derive(Debug, Serialize, Deserialize)]
pub struct ShredResult {
    pub path: String,
    pub success: bool,
    pub error: Option<String>,
    pub mb: f64,
    pub speed_mb_s: f64,
}

/// Returns true when `path` is a drive/volume root (e.g. `D:\` or `/mnt/usb`).
pub fn is_volume_root(path: &Path) -> bool {
    let s = path.to_string_lossy();
    #[cfg(windows)]
    {
        return s.len() == 3
            && s.as_bytes().get(1) == Some(&b':')
            && s.as_bytes().get(2) == Some(&b'\\');
    }
    #[cfg(not(windows))]
    {
        path.is_absolute() && path.components().count() <= 2
    }
}

pub fn shred_path(path: &Path, opts: &ShredOptions) -> anyhow::Result<Vec<ShredResult>> {
    block_protected_paths::assert_safe(path)?;

    let mut results = if path.is_file() {
        vec![shred_one(path, opts)]
    } else if path.is_dir() {
        let mut files: Vec<PathBuf> = Vec::new();
        collect_files(path, &mut files)?;

        let file_results: Vec<ShredResult> =
            files.par_iter().map(|f| shred_one(f, opts)).collect();

        remove_dirs(path)?;
        file_results
    } else {
        anyhow::bail!("Path {:?} does not exist or is not a regular file/directory", path);
    };

    let full_wipe = opts.wipe_free_space || opts.full_drive;
    if full_wipe && path.is_dir() {
        results.extend(run_volume_finalize(path)?);
    }

    Ok(results)
}

/// After all files are shredded: wipe free clusters, then TRIM the volume.
fn run_volume_finalize(root: &Path) -> anyhow::Result<Vec<ShredResult>> {
    let mut extra = Vec::new();

    // Phase 1 — overwrite every free cluster with zeros
    let t0 = Instant::now();
    match wipe_free_space::wipe_free_space(root) {
        Ok(bytes) => {
            let elapsed = t0.elapsed().as_secs_f64();
            let mb = bytes as f64 / 1_048_576.0;
            extra.push(ShredResult {
                path: format!("{} [free space]", root.display()),
                success: true,
                error: None,
                mb,
                speed_mb_s: if elapsed > 0.0 { mb / elapsed } else { f64::INFINITY },
            });
        }
        Err(e) => {
            extra.push(ShredResult {
                path: format!("{} [free space]", root.display()),
                success: false,
                error: Some(e.to_string()),
                mb: 0.0,
                speed_mb_s: 0.0,
            });
        }
    }

    // Phase 2 — volume-level TRIM so SSD firmware releases all deallocated blocks
    match volume_trim::trim_volume(root) {
            Ok(true) => {
                extra.push(ShredResult {
                    path: format!("{} [volume TRIM]", root.display()),
                    success: true,
                    error: None,
                    mb: 0.0,
                    speed_mb_s: 0.0,
                });
            }
            Ok(false) => {}
            Err(e) => {
                extra.push(ShredResult {
                    path: format!("{} [volume TRIM]", root.display()),
                    success: false,
                    error: Some(e.to_string()),
                    mb: 0.0,
                    speed_mb_s: 0.0,
                });
            }
    }

    Ok(extra)
}

fn shred_one(path: &Path, opts: &ShredOptions) -> ShredResult {
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
        } else if !is_internal_temp_file(&path) {
            out.push(path);
        }
    }
    Ok(())
}

fn is_internal_temp_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with(".nitroshred_wipe_") && n.ends_with(".tmp"))
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
    // Never remove a volume root (e.g. D:\) — only its contents.
    if !is_volume_root(dir) {
        std::fs::remove_dir(dir).ok();
    }
    Ok(())
}
