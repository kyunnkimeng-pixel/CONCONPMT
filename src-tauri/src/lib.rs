#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::Manager;

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = app_state::AppState::initialize(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::collections::list_collections,
            commands::collections::create_collection,
            commands::collections::rename_collection,
            commands::collections::delete_collection,
            commands::collections::duplicate_collection,
            commands::collections::set_collection_cover_icon,
            commands::icons::list_icons,
            commands::icons::update_icon_piece_alt,
            commands::icons::duplicate_icon,
            commands::icons::delete_icons,
            commands::icons::reorder_icons,
            commands::editor::get_icon_editor_state,
            commands::editor::apply_icon_crop,
            commands::export::list_export_profiles,
            commands::export::save_export_profile_settings,
            commands::export::validate_export_collection,
            commands::export::export_collection,
            commands::export::open_export_path,
            commands::import::import_image_files,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

mod app_state;
mod commands;
mod db;
mod error;
mod export;
mod ids;
mod imaging;
mod models;
mod paths;
