use nitroshred_core::{
    bootable_script, hardware_secure_erase, physical_drive_wipe, ShredOptions, ShredResult,
    shred_path,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{atomic::AtomicBool, Arc};
use tauri::Emitter;

#[derive(Debug, Deserialize)]
pub struct ShredRequest {
    pub paths: Vec<String>,
    pub full_drive: bool,
}

#[derive(Debug, Serialize)]
pub struct ShredResponse {
    pub results: Vec<ShredResult>,
    pub total_mb: f64,
    pub avg_speed_mb_s: f64,
}

#[derive(Debug, Serialize)]
pub struct PathInfo {
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub exists: bool,
}

#[derive(Debug, Serialize)]
pub struct DriveInfo {
    pub letter: String,
    pub path: String,
    pub label: String,
    pub total_bytes: u64,
    pub is_system: bool,
}

#[tauri::command]
pub fn shred(req: ShredRequest) -> Result<ShredResponse, String> {
    let opts = ShredOptions {
        verbose: false,
        wipe_free_space: req.full_drive,
        full_drive: req.full_drive,
    };

    let mut all_results: Vec<ShredResult> = Vec::new();

    for path_str in &req.paths {
        let path = Path::new(path_str);
        match shred_path(path, &opts) {
            Ok(results) => all_results.extend(results),
            Err(e) => all_results.push(ShredResult {
                path: path_str.clone(),
                success: false,
                error: Some(e.to_string()),
                mb: 0.0,
                speed_mb_s: 0.0,
            }),
        }
    }

    let total_mb: f64 = all_results.iter().filter(|r| r.success).map(|r| r.mb).sum();
    let speeds: Vec<f64> = all_results
        .iter()
        .filter(|r| r.success && r.speed_mb_s.is_finite())
        .map(|r| r.speed_mb_s)
        .collect();
    let avg_speed_mb_s = if speeds.is_empty() {
        0.0
    } else {
        speeds.iter().sum::<f64>() / speeds.len() as f64
    };

    Ok(ShredResponse {
        results: all_results,
        total_mb,
        avg_speed_mb_s,
    })
}

#[tauri::command]
pub fn validate_path(path: String) -> Result<PathInfo, String> {
    let p = Path::new(&path);
    validate_target(p)?;
    path_info(p)
}

#[tauri::command]
pub fn list_drives() -> Result<Vec<DriveInfo>, String> {
    list_available_drives()
}

fn validate_target(p: &Path) -> Result<(), String> {
    nitroshred_core::block_protected_paths::assert_safe(p).map_err(|e| e.to_string())?;

    if !p.exists() {
        return Err(format!("Path does not exist: {}", p.display()));
    }

    if p.is_file() {
        return Err("Select a folder or drive, not a single file.".into());
    }

    Ok(())
}

fn path_info(p: &Path) -> Result<PathInfo, String> {
    let metadata = p.metadata().map_err(|e| e.to_string())?;
    Ok(PathInfo {
        path: p.display().to_string(),
        is_dir: metadata.is_dir(),
        size_bytes: if metadata.is_dir() {
            0
        } else {
            metadata.len()
        },
        exists: true,
    })
}

fn list_available_drives() -> Result<Vec<DriveInfo>, String> {
    #[cfg(windows)]
    {
        let mut drives = Vec::new();
        for letter in b'A'..=b'Z' {
            let letter = letter as char;
            let path = format!("{letter}:\\");
            if !Path::new(&path).exists() {
                continue;
            }
            let is_system = letter == 'C';
            drives.push(DriveInfo {
                letter: format!("{letter}:"),
                path: path.clone(),
                label: windows_volume_label(&path).unwrap_or_else(|| "Local Disk".into()),
                total_bytes: windows_drive_total_bytes(&path),
                is_system,
            });
        }
        if drives.is_empty() {
            return Err("No drives found.".into());
        }
        Ok(drives)
    }

    #[cfg(target_os = "linux")]
    {
        linux_list_mounts()
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Ok(vec![DriveInfo {
            letter: "/".into(),
            path: "/".into(),
            label: "Root".into(),
            total_bytes: 0,
            is_system: true,
        }])
    }
}

#[cfg(target_os = "linux")]
fn linux_list_mounts() -> Result<Vec<DriveInfo>, String> {
    let mounts = std::fs::read_to_string("/proc/mounts")
        .or_else(|_| std::fs::read_to_string("/etc/mtab"))
        .map_err(|e| e.to_string())?;

    let mut drives = Vec::new();
    for line in mounts.lines() {
        let mut parts = line.split_whitespace();
        let Some(_device) = parts.next() else { continue };
        let Some(mount) = parts.next() else { continue };

        if mount == "/" {
            continue;
        }
        if !(mount.starts_with("/media/")
            || mount.starts_with("/mnt/")
            || mount.starts_with("/run/media/"))
        {
            continue;
        }

        let path = Path::new(mount);
        if !path.is_dir() {
            continue;
        }

        let label = mount.rsplit('/').next().unwrap_or(mount).to_string();
        drives.push(DriveInfo {
            letter: mount.to_string(),
            path: mount.to_string(),
            label,
            total_bytes: linux_mount_total_bytes(mount),
            is_system: false,
        });
    }

    if drives.is_empty() {
        return Err(
            "No removable drives found. Plug in a USB drive (usually mounted under /media or /mnt)."
                .into(),
        );
    }
    Ok(drives)
}

#[cfg(target_os = "linux")]
fn linux_mount_total_bytes(mount: &str) -> u64 {
    let output = match std::process::Command::new("df")
        .args(["-B1", mount])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return 0,
    };
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .nth(1)
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

#[cfg(windows)]
fn windows_drive_total_bytes(root: &str) -> u64 {
    use std::ffi::OsStr;
    use std::mem::MaybeUninit;
    use std::os::windows::ffi::OsStrExt;

    let wide: Vec<u16> = OsStr::new(root)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut total = MaybeUninit::<u64>::uninit();
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            wide.as_ptr(),
            std::ptr::null_mut(),
            total.as_mut_ptr(),
            std::ptr::null_mut(),
        )
    };
    if ok != 0 {
        unsafe { total.assume_init() }
    } else {
        0
    }
}

#[cfg(windows)]
fn windows_volume_label(root: &str) -> Option<String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let wide: Vec<u16> = OsStr::new(root)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut label = [0u16; 261];

    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW(
            wide.as_ptr(),
            label.as_mut_ptr(),
            label.len() as u32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        )
    };
    if ok == 0 {
        return None;
    }

    let len = label.iter().position(|&c| c == 0).unwrap_or(0);
    Some(String::from_utf16_lossy(&label[..len]))
}

// ═══════════════════════════════════════════════════════════════════════════
//  Physical-drive commands
// ═══════════════════════════════════════════════════════════════════════════

/// Enumerate all physical block devices visible to the OS.
#[tauri::command]
pub fn list_physical_drives(
) -> Result<Vec<physical_drive_wipe::PhysicalDriveInfo>, String> {
    physical_drive_wipe::list_physical_drives().map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
pub struct RawWipeRequest {
    pub drive_path: String,
}

#[derive(Debug, Serialize)]
pub struct RawWipeResult {
    pub bytes_wiped: u64,
    pub mb_wiped: f64,
    pub speed_mb_s: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WipeProgressEvent {
    pub bytes_done: u64,
    pub total_bytes: u64,
    pub pct: u8,
    pub speed_mb_s: f64,
}

/// Zero-fill every sector on a physical drive.
/// Emits `physical-wipe-progress` events during the wipe.
/// Requires admin / root privileges.
#[tauri::command]
pub async fn raw_sector_wipe(
    app: tauri::AppHandle,
    req: RawWipeRequest,
) -> Result<RawWipeResult, String> {
    // Guard: refuse system drives
    let drives = physical_drive_wipe::list_physical_drives().map_err(|e| e.to_string())?;
    if let Some(d) = drives.iter().find(|d| d.path == req.drive_path) {
        if d.is_system {
            return Err("System drive protection: cannot wipe the OS drive.".into());
        }
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel::<WipeProgressEvent>(64);
    let cancel = Arc::new(AtomicBool::new(false));
    let drive_path = req.drive_path.clone();
    let start = std::time::Instant::now();

    let mut wipe_task = tokio::task::spawn_blocking(move || {
        physical_drive_wipe::raw_wipe_physical_drive(
            &drive_path,
            cancel,
            move |bytes_done, total_bytes, speed_mb_s| {
                let pct = if total_bytes > 0 {
                    ((bytes_done as f64 / total_bytes as f64) * 100.0).min(100.0) as u8
                } else {
                    0
                };
                let _ = tx.blocking_send(WipeProgressEvent {
                    bytes_done,
                    total_bytes,
                    pct,
                    speed_mb_s,
                });
            },
        )
    });

    // Stream progress events while wipe runs
    loop {
        tokio::select! {
            maybe_evt = rx.recv() => {
                match maybe_evt {
                    Some(evt) => { app.emit("physical-wipe-progress", &evt).ok(); }
                    None => break,
                }
            }
            result = &mut wipe_task => {
                while let Ok(evt) = rx.try_recv() {
                    app.emit("physical-wipe-progress", &evt).ok();
                }
                let bytes_wiped = result
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())?;
                let elapsed = start.elapsed().as_secs_f64();
                let mb_wiped = bytes_wiped as f64 / 1_048_576.0;
                return Ok(RawWipeResult {
                    bytes_wiped,
                    mb_wiped,
                    speed_mb_s: if elapsed > 0.0 { mb_wiped / elapsed } else { 0.0 },
                });
            }
        }
    }

    Err("Wipe task ended unexpectedly".into())
}

#[derive(Debug, Deserialize)]
pub struct HardwareEraseRequest {
    pub drive_path: String,
    /// "nvme_crypto" | "nvme_block"
    pub method: String,
}

#[derive(Debug, Serialize)]
pub struct HardwareEraseResult {
    pub accepted: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SecureEraseCapResult {
    pub nvme_sanitize_crypto: bool,
    pub nvme_sanitize_block: bool,
    pub ata_secure_erase: bool,
    pub ata_enhanced_erase: bool,
    pub ata_frozen: bool,
}

/// Query hardware-level secure erase capabilities for a physical drive.
#[tauri::command]
pub fn query_secure_erase_capability(
    drive_path: String,
) -> Result<SecureEraseCapResult, String> {
    hardware_secure_erase::query_capability(&drive_path)
        .map(|c| SecureEraseCapResult {
            nvme_sanitize_crypto: c.nvme_sanitize_crypto,
            nvme_sanitize_block: c.nvme_sanitize_block,
            ata_secure_erase: c.ata_secure_erase,
            ata_enhanced_erase: c.ata_enhanced_erase,
            ata_frozen: c.ata_frozen,
        })
        .map_err(|e| e.to_string())
}

/// Issue an NVMe Sanitize command to the drive.
/// Returns immediately — the firmware performs the erase asynchronously.
#[tauri::command]
pub fn hardware_secure_erase(req: HardwareEraseRequest) -> Result<HardwareEraseResult, String> {
    let action = match req.method.as_str() {
        "nvme_crypto" => hardware_secure_erase::NvmeSanitizeAction::CryptoErase,
        "nvme_block" => hardware_secure_erase::NvmeSanitizeAction::BlockErase,
        other => return Err(format!("Unknown erase method: {:?}", other)),
    };

    hardware_secure_erase::nvme_sanitize(&req.drive_path, action)
        .map(|()| HardwareEraseResult {
            accepted: true,
            message: "Sanitize command accepted. The drive firmware is now erasing data \
                      in the background. This can take from seconds (crypto erase) to \
                      several minutes (block erase). You may monitor status below."
                .into(),
        })
        .map_err(|e| e.to_string())
}

/// Poll NVMe Sanitize progress for a drive.
/// Returns 0–100 while in progress, None if not running / already complete.
#[tauri::command]
pub fn nvme_sanitize_status(drive_path: String) -> Result<Option<u8>, String> {
    hardware_secure_erase::nvme_sanitize_status(&drive_path).map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
pub struct ExportScriptRequest {
    /// Absolute path to the output directory.
    pub output_dir: String,
    /// Device paths to include in the script, e.g. ["/dev/sda"].
    pub device_paths: Vec<String>,
    /// Number of wipe passes (1 = single zero-fill, 3 = DoD).
    pub passes: u8,
}

/// Generate and save a bootable wipe script package to `output_dir`.
#[tauri::command]
pub fn export_bootable_script(req: ExportScriptRequest) -> Result<String, String> {
    let device_refs: Vec<&str> = req.device_paths.iter().map(|s| s.as_str()).collect();
    let out = std::path::Path::new(&req.output_dir);
    bootable_script::save_bootable_package(out, &device_refs, req.passes)
        .map(|()| {
            format!(
                "Saved nitroshred-wipe.sh and README.txt to {}",
                req.output_dir
            )
        })
        .map_err(|e| e.to_string())
}
