mod cache;
mod cf;
mod convert;
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
            operations::apply_selected_version,
            operations::list_versions,
            operations::get_project_options,
            operations::update_dependencies_batch,
            operations::get_log_dir,
            operations::apply_selected_versions_batch,
            operations::get_batch_mod_briefs,
            operations::save_log,
            operations::clear_all_caches,
            operations::refresh_mojang_cache,
            convert::convert_aw_at
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
