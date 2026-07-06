mod commands;

use commands::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_data_dir = dirs_next::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("yse");

    std::fs::create_dir_all(&app_data_dir).ok();

    let db_path = app_data_dir.join("yse.db");

    let state = AppState::new(db_path).expect("failed to initialize YSE application state");

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_os::init());

    #[cfg(mobile)]
    let builder = builder.plugin(tauri_plugin_barcode_scanner::init());

    builder
        .manage(state)
        .setup(|app| {
            let state = app.state::<AppState>();
            let handle = app.handle().clone();
            *state.app_handle.lock().unwrap() = Some(handle.clone());
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async {
                    state.core.load_config().await;
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::send_message,
            commands::get_messages,
            commands::get_config,
            commands::save_config,
            commands::start_polling,
            commands::auto_start_plugins,
            commands::stop_polling,
            commands::get_hostname,
            commands::toggle_hide_conversation,
            commands::get_hidden_addresses,
            commands::delete_conversation,
            commands::get_contact_hashes,
            commands::get_known_hostnames,
            commands::get_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running yse mobile");
}
