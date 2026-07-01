use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::Instant;

const CHUNK: usize = 16 * 1024 * 1024; // 16 MB — large writes saturate disk bandwidth

fn is_disk_full(err: &std::io::Error) -> bool {
    if err.kind() == std::io::ErrorKind::StorageFull {
        return true;
    }
    #[cfg(windows)]
    if err.raw_os_error() == Some(112) {
        // ERROR_DISK_FULL
        return true;
    }
    #[cfg(unix)]
    if err.raw_os_error() == Some(libc::ENOSPC) {
        return true;
    }
    false
}

/// Fill all remaining free space on the volume with zeros, then delete the temp file.
/// This overwrites previously-deleted file data still sitting in free clusters.
pub fn wipe_free_space(root: &Path) -> std::io::Result<u64> {
    let temp = root.join(format!(".nitroshred_wipe_{:08x}.tmp", rand::random::<u32>()));
    let t0 = Instant::now();

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;

    let buf = vec![0u8; CHUNK];
    let mut written = 0u64;

    loop {
        match file.write(&buf) {
            Ok(0) => break,
            Ok(n) => written += n as u64,
            Err(e) if is_disk_full(&e) => break,
            Err(e) => {
                let _ = fs::remove_file(&temp);
                return Err(e);
            }
        }
    }

    file.sync_all()?;
    drop(file);
    fs::remove_file(&temp)?;

    let elapsed = t0.elapsed().as_secs_f64();
    if elapsed > 0.0 {
        let mb_s = (written as f64 / 1_048_576.0) / elapsed;
        eprintln!(
            "[nitroshred] free-space wipe  {:.1} MB  {:.0} MB/s",
            written as f64 / 1_048_576.0,
            mb_s
        );
    }

    Ok(written)
}
