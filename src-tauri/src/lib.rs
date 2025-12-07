mod cf;
mod gradle;
mod mojang;
mod mr;
mod operations;
mod util;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|_app| {
            tauri::async_runtime::spawn(crate::mojang::refresh_manifest_cache_on_startup());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            operations::update_dependency,
            operations::get_project_options,
            operations::update_dependencies_batch,
            operations::save_log
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
