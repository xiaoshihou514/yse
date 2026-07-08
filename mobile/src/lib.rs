mod commands;

use commands::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Debug)
                .clear_targets()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir { file_name: None },
                ))
                .target(
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview)
                        .format(|out, _message, record| {
                            out.finish(format_args!("{}", record.args()))
                        }),
                )
                .build(),
        )
        .plugin(tauri_plugin_dialog::init());

    #[cfg(mobile)]
    let builder = builder.plugin(tauri_plugin_barcode_scanner::init());

    builder
        .setup(|app| {
            // Use Tauri's path API (works on Android, unlike dirs_next)
            let app_dir = app.path().app_data_dir().unwrap_or_else(|_| {
                // Fallback for environments where path resolver is unavailable
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            });
            std::fs::create_dir_all(&app_dir).ok();
            let db_path = app_dir.join("yse.db");
            eprintln!("yse: using app_dir={:?} db_path={:?}", app_dir, db_path);

            let state = AppState::new(&db_path).expect("yse mobile: AppState::new failed");
            *state.app_handle.lock().unwrap() = Some(app.handle().clone());

            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async {
                    state.core.load_config().await;
                });
            }

            app.manage(state);
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
            commands::set_local_hostname,
            commands::toggle_hide_conversation,
            commands::get_hidden_addresses,
            commands::delete_conversation,
            commands::get_contact_hashes,
            commands::get_known_hostnames,
        ])
        .run(tauri::generate_context!())
        .expect("error while running yse mobile");
}
