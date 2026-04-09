mod commands;
mod settings;
mod steam;
mod support;

pub(crate) fn launcher_updater_pubkey() -> Option<&'static str> {
    match option_env!("SRC_LAUNCHER_UPDATER_PUBKEY") {
        Some(value) if !value.trim().is_empty() => Some(value),
        _ => None,
    }
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if let Some(pubkey) = launcher_updater_pubkey() {
                #[cfg(desktop)]
                app.handle()
                    .plugin(tauri_plugin_updater::Builder::new().pubkey(pubkey).build())?;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_settings,
            commands::save_settings,
            commands::detect_subrosa,
            commands::append_launcher_log,
            commands::open_launcher_logs,
            commands::open_client_crashlogs_folder,
            commands::open_client_config_folder,
            commands::open_cache_folder,
            commands::force_redownload,
            commands::clear_cache,
            commands::collect_launcher_diagnostics,
            commands::collect_client_diagnostics,
            commands::copy_text_to_clipboard,
            commands::get_launcher_update_state,
            commands::install_launcher_update,
            commands::get_release_version,
            commands::download_injection_library,
            commands::launch_game
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
