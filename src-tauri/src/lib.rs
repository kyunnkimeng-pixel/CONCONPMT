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
            commands::collections::update_collection_settings,
            commands::collections::import_collection_cover_image,
            commands::icons::list_icons,
            commands::icons::update_icon_piece_alt,
            commands::icons::rename_icon,
            commands::icons::set_icon_thumbnail_override,
            commands::icons::duplicate_icon,
            commands::icons::delete_icons,
            commands::icons::reorder_icons,
            commands::icons::reveal_icon_original,
            commands::icons::reveal_icon_export_result,
            commands::editor::get_icon_editor_state,
            commands::editor::apply_icon_crop,
            commands::export::list_export_profiles,
            commands::export::save_export_profile_settings,
            commands::export::validate_export_collection,
            commands::export::export_collection,
            commands::export::open_export_path,
            commands::export::pick_export_directory,
            commands::import::import_image_files,
            commands::settings::get_app_settings,
            commands::settings::save_app_settings,
            commands::library::preview_library_cleanup,
            commands::library::cleanup_library,
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
