use rayon::prelude::*;
use std::fs::OpenOptions;
use std::os::unix::fs::FileExt;
use std::path::Path;

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
                file.write_at(&buf[..chunk], offset + done)?;
                done += chunk as u64;
            }
            Ok(())
        })
        .collect::<std::io::Result<Vec<_>>>()?;

    Ok(())
}
