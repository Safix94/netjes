pub mod categories;
pub mod commands;
pub mod dupes;
pub mod mover;
pub mod paths;
pub mod planner;
pub mod scan;
pub mod state;
pub mod thumbs;
pub mod undo;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_downloads_dir,
            commands::get_categories,
            commands::scan,
            commands::begin_organize,
            commands::move_files,
            commands::check_undo,
            commands::undo_organize,
            commands::disorganize,
            commands::list_folder_files,
            commands::get_thumbnails,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
