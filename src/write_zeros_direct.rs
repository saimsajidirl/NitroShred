use std::fs::{remove_file, rename, OpenOptions};
use std::io::Write;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

const BUF_SIZE: usize = 8 * 1024 * 1024; // 8 MB — reduces syscalls 128× vs 64 KB

pub fn zero_fill(path: &Path) -> std::io::Result<()> {
    let mut opts = OpenOptions::new();
    opts.write(true);

    #[cfg(unix)]
    opts.custom_flags(libc::O_DIRECT);

    let mut file = opts.open(path)?;
    let total = file.metadata()?.len();

    let buffer = vec![0u8; BUF_SIZE];
    let mut written = 0u64;

    while written < total {
        let chunk = ((total - written) as usize).min(BUF_SIZE);
        file.write_all(&buffer[..chunk])?;
        written += chunk as u64;
    }

    file.sync_all()?;
    Ok(())
}

pub fn scramble_metadata(path: &Path) -> std::io::Result<()> {
    // truncate → rename → unlink
    let file = OpenOptions::new().write(true).open(path)?;
    file.set_len(0)?;
    drop(file);

    let scrambled = path.with_file_name(format!("ns_{:08x}", rand::random::<u32>()));
    rename(path, &scrambled)?;
    remove_file(scrambled)?;
    Ok(())
}
