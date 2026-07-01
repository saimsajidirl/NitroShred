use std::path::Path;
use std::time::Instant;

use crate::write_zeros_parallel::{parallel_shred, PARALLEL_THRESHOLD};
use crate::write_zeros_direct::{scramble_metadata, zero_fill};
use crate::trim_ssd_blocks::try_trim;

pub struct ShredOptions {
    pub force: bool,
    pub verbose: bool,
    pub no_trim: bool,
}

pub fn shred_file(path: &Path, opts: &ShredOptions) -> anyhow::Result<()> {
    if opts.force {
        // attempt to make writable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = path.metadata()?.permissions();
            perms.set_mode(perms.mode() | 0o200);
            std::fs::set_permissions(path, perms).ok();
        }
        #[cfg(windows)]
        {
            let mut perms = path.metadata()?.permissions();
            perms.set_readonly(false);
            std::fs::set_permissions(path, perms).ok();
        }
    }

    let size = path.metadata()?.len();
    let t0 = Instant::now();

    let path_used = select_io_path(path, size, opts)?;

    if opts.verbose {
        let elapsed = t0.elapsed().as_secs_f64();
        let mb = size as f64 / 1_048_576.0;
        let speed = if elapsed > 0.0 { mb / elapsed } else { f64::INFINITY };
        eprintln!(
            "[nitroshred] {:?}  {:.1} MB  {:.0} MB/s  path={}",
            path,
            mb,
            speed,
            path_used
        );
    }

    // Async metadata pipeline — scramble runs after I/O but overlapped with next file via rayon
    scramble_metadata(path)?;
    Ok(())
}

fn select_io_path(path: &Path, size: u64, opts: &ShredOptions) -> anyhow::Result<&'static str> {
    // 1. TRIM — primary on SSD/NVMe unless suppressed
    if !opts.no_trim {
        let trimmed = try_trim(path)?;
        if trimmed {
            return Ok("TRIM");
        }
    }

    // 2. Intra-file parallel pwrite for large files
    if size >= PARALLEL_THRESHOLD {
        parallel_shred(path)?;
        return Ok("parallel-pwrite");
    }

    // 3. io_uring on Linux
    #[cfg(target_os = "linux")]
    {
        crate::write_zeros_uring::shred_uring(path)?;
        return Ok("io_uring");
    }

    // 4. Fallback: O_DIRECT zero-fill (Windows / non-uring Linux)
    #[allow(unreachable_code)]
    {
        zero_fill(path)?;
        Ok("zero-fill")
    }
}
