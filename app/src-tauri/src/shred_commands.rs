use nitroshred_core::{ShredOptions, ShredResult, shred_path};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct ShredRequest {
    pub paths: Vec<String>,
    pub force: bool,
    pub no_trim: bool,
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
        force: req.force,
        verbose: false,
        no_trim: req.no_trim,
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

    #[cfg(not(windows))]
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
