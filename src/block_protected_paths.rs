use std::path::Path;

static BLOCKED_PATHS: &[&str] = &[
    "/",
    "/bin", "/boot", "/dev", "/etc", "/lib", "/lib64",
    "/proc", "/run", "/sbin", "/sys", "/usr",
];

#[cfg(target_os = "windows")]
static BLOCKED_PATHS_WIN: &[&str] = &[
    "C:\\", "C:\\Windows", "C:\\Windows\\System32",
    "C:\\Program Files", "C:\\Program Files (x86)",
];

pub fn assert_safe(path: &Path) -> anyhow::Result<()> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let s = canonical.to_string_lossy();

    for blocked in BLOCKED_PATHS {
        if s == *blocked {
            anyhow::bail!("Refusing to shred protected path: {}", s);
        }
    }

    #[cfg(target_os = "windows")]
    for blocked in BLOCKED_PATHS_WIN {
        if s.eq_ignore_ascii_case(blocked) {
            anyhow::bail!("Refusing to shred protected path: {}", s);
        }
    }

    Ok(())
}
