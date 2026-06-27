use std::path::Path;

#[cfg(target_os = "linux")]
pub fn try_trim(path: &Path) -> std::io::Result<bool> {
    use std::fs::OpenOptions;
    use std::os::unix::io::AsRawFd;

    let file = OpenOptions::new().write(true).open(path)?;
    let size = file.metadata()?.len();

    if size == 0 {
        return Ok(true);
    }

    // fallocate PUNCH_HOLE instructs the filesystem to release the physical blocks
    // backing this file range. On SSDs, the filesystem passes TRIM commands to the
    // drive controller for exactly those sectors — no manual extent mapping needed.
    let ret = unsafe {
        libc::fallocate(
            file.as_raw_fd(),
            libc::FALLOC_FL_PUNCH_HOLE | libc::FALLOC_FL_KEEP_SIZE,
            0,
            size as libc::off_t,
        )
    };

    if ret == 0 {
        file.sync_all()?;
        Ok(true)
    } else {
        // EOPNOTSUPP: filesystem doesn't support hole punching (e.g. FAT, older kernels)
        // Fall through to zero-fill path.
        match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::EOPNOTSUPP) | Some(libc::ENOSYS) | Some(libc::EINVAL) => Ok(false),
            _ => Err(std::io::Error::last_os_error()),
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn try_trim(_path: &Path) -> std::io::Result<bool> {
    Ok(false)
}
