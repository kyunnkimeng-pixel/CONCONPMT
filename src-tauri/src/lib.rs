#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::Manager;

    let builder = tauri::Builder::default();
    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }));

    builder
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = app_state::AppState::initialize(app.handle())?;
            state.start_ai_handoff_maintenance_worker();
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::collections::list_collections,
            commands::collections::create_collection,
            commands::ai::get_ai_review_state,
            commands::ai::get_ai_provider_session_status,
            commands::ai::set_ai_session_credential,
            commands::ai::clear_ai_session_credential,
            commands::ai::execute_ai_image_edit,
            commands::ai::open_ai_official_resource,
            commands::ai::import_local_ai_candidate,
            commands::ai::preview_ai_candidate_normalization,
            commands::ai::activate_ai_candidate,
            commands::ai::create_ai_icon_root,
            commands::ai::restore_ai_version,
            commands::ai::repair_ai_to_original,
            commands::ai_grid::prepare_ai_grid_edit_workspace,
            commands::ai_grid::prepare_ai_generation_workspace,
            commands::ai_grid::get_ai_grid_workspace,
            commands::ai_grid::get_latest_ai_grid_workspace,
            commands::ai_grid::mark_ai_grid_workspace_awaiting_result,
            commands::ai_grid::attach_ai_grid_output,
            commands::ai_grid::analyze_ai_grid_output,
            commands::ai_grid::commit_ai_grid_review,
            commands::ai_grid::commit_ai_generated_icons,
            commands::ai_grid::cancel_ai_grid_workspace,
            commands::ai_grid::reveal_ai_grid_input,
            commands::ai_grid::start_ai_grid_input_drag,
            commands::ai_handoff::prepare_ai_web_handoff,
            commands::ai_handoff::get_ai_web_handoff,
            commands::ai_handoff::get_latest_ai_web_handoff_for_icon,
            commands::ai_handoff::reveal_ai_web_handoff_upload,
            commands::ai_handoff::start_ai_web_handoff_drag,
            commands::ai_handoff::validate_ai_web_handoff_result,
            commands::ai_handoff::commit_ai_web_handoff_result,
            commands::ai_handoff::extend_ai_web_handoff_retention,
            commands::ai_handoff::delete_ai_web_handoff_payload,
            commands::ai_handoff::list_recent_ai_web_handoffs,
            commands::ai_handoff::get_ai_web_handoff_storage_status,
            commands::ai_handoff::run_ai_web_handoff_maintenance,
            commands::collections::rename_collection,
            commands::collections::delete_collection,
            commands::collections::duplicate_collection,
            commands::collections::set_collection_cover_icon,
            commands::collections::update_collection_settings,
            commands::collections::import_collection_cover_image,
            commands::icons::list_icons,
            commands::icons::update_icon_piece_alt,
            commands::icons::rename_icon,
            commands::icons::get_icon_note,
            commands::icons::update_icon_note,
            commands::icons::clear_icon_note,
            commands::icons::set_icon_thumbnail_override,
            commands::icons::create_placeholder_icon,
            commands::icons::replace_icon_source,
            commands::icons::set_icons_readiness,
            commands::icons::duplicate_icon,
            commands::icons::delete_icons,
            commands::icons::reorder_icons,
            commands::icons::reveal_icon_original,
            commands::icons::reveal_icon_export_result,
            commands::editor::get_icon_editor_state,
            commands::editor::apply_icon_crop,
            commands::editor::update_icon_text_overlay,
            commands::editor::preview_icon_effects,
            commands::editor::update_icon_effects,
            commands::editor::preview_icon_motion,
            commands::editor::update_icon_motion,
            commands::editor::pick_text_overlay_font,
            commands::export::list_export_profiles,
            commands::export::save_export_profile_settings,
            commands::export::validate_export_collection,
            commands::export::export_collection,
            commands::export::export_selected_collection_items,
            commands::export::open_export_path,
            commands::export::pick_export_directory,
            commands::export::analyze_export_asset_candidate,
            commands::export::generate_gif_optimization_candidates,
            commands::export::generate_static_optimization_candidates,
            commands::export::list_optimization_candidates,
            commands::export::apply_optimization_candidate,
            commands::export::apply_optimization_candidate_to_preview,
            commands::export::preview_gif_playback_fps,
            commands::export::apply_gif_original_playback_to_preview,
            commands::export::clear_optimization_candidate,
            commands::export::revalidate_export_item,
            commands::export::get_active_export_variant,
            commands::import::import_image_files,
            commands::settings::get_app_settings,
            commands::settings::save_app_settings,
            commands::library::preview_library_cleanup,
            commands::library::cleanup_library,
            commands::sheet::analyze_sheet_grid,
            commands::sheet::preview_sheet_slices,
            commands::sheet::auto_detect_sheet_grid,
            commands::sheet::import_sheet_cells,
            commands::sheet::measure_frame_sheet_gif,
            commands::sheet::create_frame_sheet_gif,
            commands::sheet::analyze_manual_slices,
            commands::sheet::import_manual_slices,
            commands::sheet::save_manual_slices,
            commands::sheet::load_manual_slices,
            commands::sheet::export_edit_sheet,
            commands::sheet::reimport_edit_sheet,
            commands::sheet::analyze_gif_frame_sheet_export,
            commands::sheet::export_gif_frame_sheet,
            commands::sheet::start_gif_frame_sheet_page_drag,
            commands::sheet::reveal_gif_frame_sheet_page,
            commands::sheet::validate_gif_frame_sheet_reimport,
            commands::sheet::reimport_gif_frame_sheet,
            commands::sheet::list_sheet_grid_presets,
            commands::sheet::create_sheet_grid_preset,
            commands::sheet::update_sheet_grid_preset,
            commands::sheet::delete_sheet_grid_preset,
            commands::sheet::duplicate_sheet_grid_preset,
            commands::sheet::set_default_sheet_grid_preset,
            commands::sheet::get_default_sheet_grid_preset,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

mod ai_provider;
mod app_state;
mod commands;
mod db;
mod error;
mod export;
mod ids;
mod imaging;
mod models;
mod native_drag;
mod optimization;
mod paths;
mod sheet;
