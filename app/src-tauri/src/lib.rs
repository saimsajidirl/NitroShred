mod shred_commands;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            shred_commands::shred,
            shred_commands::validate_path,
            shred_commands::list_drives,
            // Physical-drive operations
            shred_commands::list_physical_drives,
            shred_commands::raw_sector_wipe,
            shred_commands::query_secure_erase_capability,
            shred_commands::hardware_secure_erase,
            shred_commands::nvme_sanitize_status,
            shred_commands::export_bootable_script,
        ])
        .setup(|app| {
            #[cfg(debug_assertions)]
            app.get_webview_window("main").unwrap().open_devtools();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running nitroshred app");
}
