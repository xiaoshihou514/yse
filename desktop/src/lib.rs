mod commands;

use commands::YseState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "yse=info".into()),
        )
        .init();

    let app_data_dir = dirs_next::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("yse");

    std::fs::create_dir_all(&app_data_dir).ok();

    let db_path = app_data_dir.join("yse.db");

    let state = YseState::new(db_path).expect("failed to initialize YSE application state");
    state.setup_plugin_handler();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .setup(|app| {
            let state = app.state::<YseState>();
            let handle = app.handle().clone();
            // Store app handle for plugin handler to emit events
            *state.app_handle.lock().unwrap() = Some(handle.clone());
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async {
                    state.load_config().await;
                    state.auto_start_plugins().await;
                    // Auto-start polling (fails gracefully if crypto key not set)
                    if let Err(e) = state.start_polling_inner(handle).await {
                        state.log("warn", format!("auto-start polling skipped: {}", e));
                    }
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
            commands::stop_polling,
            commands::list_plugins,
            commands::add_plugin,
            commands::remove_plugin,
            commands::toggle_plugin,
            commands::start_plugin,
            commands::stop_plugin,
            commands::list_running_plugins,
            commands::get_logs,
            commands::test_email,
        ])
        .run(tauri::generate_context!())
        .expect("error while running yse desktop");
}
