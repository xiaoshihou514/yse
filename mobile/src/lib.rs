use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_data_dir = dirs_next::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("yse");

    std::fs::create_dir_all(&app_data_dir).ok();

    let db_path = app_data_dir.join("yse.db");

    let state = yse_desktop::commands::YseState::new(db_path)
        .expect("failed to initialize YSE application state");

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_os::init());

    #[cfg(mobile)]
    let builder = builder.plugin(tauri_plugin_barcode_scanner::init());

    builder
        .manage(state)
        .setup(|app| {
            let state = app.state::<yse_desktop::commands::YseState>();
            let handle = app.handle().clone();
            *state.app_handle.lock().unwrap() = Some(handle.clone());
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async {
                    state.load_config().await;
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            yse_desktop::commands::send_message,
            yse_desktop::commands::get_messages,
            yse_desktop::commands::get_config,
            yse_desktop::commands::save_config,
            yse_desktop::commands::start_polling,
            yse_desktop::commands::auto_start_plugins,
            yse_desktop::commands::stop_polling,
            yse_desktop::commands::list_plugins,
            yse_desktop::commands::add_plugin,
            yse_desktop::commands::remove_plugin,
            yse_desktop::commands::toggle_plugin,
            yse_desktop::commands::start_plugin,
            yse_desktop::commands::stop_plugin,
            yse_desktop::commands::list_running_plugins,
            yse_desktop::commands::list_processes,
            yse_desktop::commands::list_sessions,
            yse_desktop::commands::get_hostname,
            yse_desktop::commands::toggle_hide_conversation,
            yse_desktop::commands::get_hidden_addresses,
            yse_desktop::commands::delete_conversation,
            yse_desktop::commands::get_contact_hashes,
            yse_desktop::commands::get_known_hostnames,
            yse_desktop::commands::get_logs,
            yse_desktop::commands::test_email,
        ])
        .run(tauri::generate_context!())
        .expect("error while running yse mobile");
}
