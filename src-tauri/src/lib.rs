mod commands;
mod settings;
mod steam;
mod support;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::load_settings,
            commands::save_settings,
            commands::detect_subrosa,
            commands::append_launcher_log,
            commands::open_logs,
            commands::open_cache_folder,
            commands::force_redownload,
            commands::clear_cache,
            commands::collect_diagnostics,
            commands::copy_text_to_clipboard,
            commands::download_injection_library,
            commands::launch_game
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
