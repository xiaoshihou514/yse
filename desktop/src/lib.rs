pub mod commands;

use commands::YseState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_data_dir = dirs_next::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("yse");

    std::fs::create_dir_all(&app_data_dir).ok();

    let db_path = app_data_dir.join("yse.db");

    let state = YseState::new(db_path).expect("failed to initialize 盐水鹅 application state");
    state.setup_plugin_handler();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_os::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .clear_targets()
                .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .target(
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview)
                        .format(|out, _message, record| {
                            out.finish(format_args!("{}", record.args()))
                        }),
                )
                .build(),
        );

    #[cfg(mobile)]
    let builder = builder.plugin(tauri_plugin_barcode_scanner::init());

    builder
        .manage(state)
        .setup(|app| {
            let state = app.state::<YseState>();
            let handle = app.handle().clone();
            // Store app handle for plugin handler to emit events
            *state.app_handle.lock().unwrap() = Some(handle.clone());
            // Use a temporary runtime only for synchronous one-time init (load config).
            // Long-lived tasks (IMAP polling, plugin stdout readers) MUST NOT
            // be spawned here — they'd be cancelled when the temp runtime drops.
            // They are started later by the frontend via Tauri commands which
            // run on the permanent runtime.
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async {
                    state.load_config().await;
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
            commands::list_plugins,
            commands::add_plugin,
            commands::remove_plugin,
            commands::toggle_plugin,
            commands::start_plugin,
            commands::stop_plugin,
            commands::list_running_plugins,
            commands::list_processes,
            commands::get_process_logs,
            commands::list_sessions,
            commands::get_hostname,
            commands::set_local_hostname,
            commands::toggle_hide_conversation,
            commands::get_hidden_addresses,
            commands::delete_conversation,
            commands::get_contact_hashes,
            commands::get_known_hostnames,
            commands::test_email,
        ])
        .run(tauri::generate_context!())
        .expect("error while running yse desktop");
}
