use tauri::Emitter;

mod commands;
mod discord;
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
        // a Discord join must not open a second launcher; the running one launches the game itself
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !discord::launch_requested_in(args) {
                return;
            }
            let settings = settings::load_settings(app).unwrap_or_default();
            if !discord::game_running(&settings.executable_name) {
                if let Err(error) = app.emit("discord-launch", ()) {
                    eprintln!("discord-launch event failed: {error}");
                }
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if let Some(pubkey) = launcher_updater_pubkey() {
                #[cfg(desktop)]
                app.handle()
                    .plugin(tauri_plugin_updater::Builder::new().pubkey(pubkey).build())?;
            }

            if discord::launch_requested() {
                let settings = settings::load_settings(app.handle()).unwrap_or_default();
                if discord::game_running(&settings.executable_name) {
                    app.handle().exit(0);
                    return Ok(());
                }
            }

            if let Err(error) = discord::register_launch_handler() {
                eprintln!("Discord launch handler not registered: {error}");
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
            commands::clear_synced_assets,
            commands::collect_launcher_diagnostics,
            commands::collect_client_diagnostics,
            commands::copy_text_to_clipboard,
            commands::get_launcher_update_state,
            commands::install_launcher_update,
            commands::get_release_version,
            commands::get_release_details,
            commands::get_release_history,
            commands::download_injection_library,
            commands::launch_game,
            discord::discord_launch_requested
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
