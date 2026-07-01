use rayon::prelude::*;
use std::fs::OpenOptions;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::FileExt;
#[cfg(windows)]
use std::os::windows::fs::FileExt;

const BUF_SIZE: usize = 8 * 1024 * 1024;
pub const PARALLEL_THRESHOLD: u64 = 512 * 1024 * 1024; // 512 MB

pub fn parallel_shred(path: &Path) -> std::io::Result<()> {
    let total = path.metadata()?.len();
    let n = rayon::current_num_threads() as u64;
    let seg = total / n;

    (0..n)
        .into_par_iter()
        .map(|i| -> std::io::Result<()> {
            let file = OpenOptions::new().write(true).open(path)?;
            let offset = i * seg;
            let len = if i == n - 1 { total - offset } else { seg };
            let buf = vec![0u8; BUF_SIZE];
            let mut done = 0u64;

            while done < len {
                let chunk = ((len - done) as usize).min(BUF_SIZE);
                #[cfg(unix)]
                file.write_at(&buf[..chunk], offset + done)?;
                #[cfg(windows)]
                file.seek_write(&buf[..chunk], offset + done)?;
                done += chunk as u64;
            }
            Ok(())
        })
        .collect::<std::io::Result<Vec<_>>>()?;

    // Single fsync after all threads complete — ensures all writes reach the device
    OpenOptions::new().write(true).open(path)?.sync_all()?;

    Ok(())
}
