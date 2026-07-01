use std::path::Path;

/// Issue a volume-level TRIM/discard after wiping so SSD controllers release all deallocated blocks.
pub fn trim_volume(root: &Path) -> std::io::Result<bool> {
    #[cfg(target_os = "linux")]
    {
        linux_fstrim(root)
    }

    #[cfg(windows)]
    {
        windows_retrim(root)
    }

    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = root;
        Ok(false)
    }
}

#[cfg(target_os = "linux")]
fn linux_fstrim(root: &Path) -> std::io::Result<bool> {
    let status = std::process::Command::new("fstrim")
        .arg("-v")
        .arg(root)
        .status()?;

    Ok(status.success())
}

#[cfg(windows)]
fn windows_retrim(root: &Path) -> std::io::Result<bool> {
    let drive = root.to_string_lossy();
    let drive = drive.trim_end_matches('\\');

    // defrag /L issues a Retrim on SSD volumes (Win8+)
    let status = std::process::Command::new("defrag")
        .args([drive, "/L", "/U"])
        .status()?;

    Ok(status.success())
}
