use std::path::Path;

#[cfg(target_os = "linux")]
pub fn try_trim(path: &Path) -> std::io::Result<bool> {
    let device = resolve_block_device(path)?;
    let status = std::process::Command::new("blkdiscard")
        .arg(&device)
        .status();

    match status {
        Ok(s) if s.success() => Ok(true),
        _ => Ok(false),
    }
}

#[cfg(target_os = "linux")]
fn resolve_block_device(path: &Path) -> std::io::Result<String> {
    // Walk /proc/mounts to find the longest prefix match, return device
    let mounts = std::fs::read_to_string("/proc/mounts")?;
    let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let abs_str = abs.to_string_lossy();

    let mut best_mount = "";
    let mut best_device = "";

    for line in mounts.lines() {
        let mut parts = line.split_whitespace();
        let device = parts.next().unwrap_or("");
        let mountpoint = parts.next().unwrap_or("");
        if abs_str.starts_with(mountpoint) && mountpoint.len() > best_mount.len() {
            best_mount = mountpoint;
            best_device = device;
        }
    }

    if best_device.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "could not resolve block device",
        ));
    }

    // blkdiscard needs the raw block device, not a partition number for file-level trim
    Ok(best_device.to_string())
}

#[cfg(not(target_os = "linux"))]
pub fn try_trim(_path: &Path) -> std::io::Result<bool> {
    // Windows FSCTL_FILE_LEVEL_TRIM — not yet implemented; fall back to zero-fill
    Ok(false)
}
