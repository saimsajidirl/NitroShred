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

    nitroshred_core::block_protected_paths::assert_safe(p).map_err(|e| e.to_string())?;

    let metadata = p.metadata().map_err(|e| e.to_string())?;
    let is_dir = metadata.is_dir();
    let size_bytes = if is_dir { 0 } else { metadata.len() };

    Ok(PathInfo {
        path: path.clone(),
        is_dir,
        size_bytes,
        exists: true,
    })
}

#[derive(Debug, Serialize)]
pub struct PathInfo {
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub exists: bool,
}
