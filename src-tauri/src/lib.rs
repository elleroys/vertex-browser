pub mod adblock;
pub mod commands;
pub mod db;
pub mod downloads;
pub mod error;
pub mod memory_saver;
pub mod password_manager;
pub mod webview_manager;

use adblock::AdBlockEngine;
use commands::AppState;
use db::Database;
use downloads::DownloadManager;
use password_manager::PasswordManager;
use std::sync::Arc;
use tauri::Manager;
use webview_manager::WebviewManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Vortex Browser: запуск...");

    #[cfg(target_os = "windows")]
    if std::env::var_os("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").is_none() {
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            webview_manager::VORTEX_BROWSER_ARGS,
        );
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            tracing::info!("Vortex setup: ініціалізація БД та сервісів");
            let app_data = app
                .path()
                .app_data_dir()
                .expect("Не вдалося отримати директорію даних додатку");
            let db_path = app_data.join("vortex.db");
            let db = Arc::new(Database::open(&db_path).expect("Помилка ініціалізації SQLite"));

            let settings = db.get_settings().unwrap_or_default();
            let adblock = Arc::new(
                AdBlockEngine::new(
                    Arc::clone(&db),
                    app_data.join("adblock_lists"),
                    settings.adblock_enabled,
                )
                .expect("Помилка ініціалізації AdBlock"),
            );

            let passwords = Arc::new(PasswordManager::new(Arc::clone(&db)));
            let download_dir = app
                .path()
                .download_dir()
                .unwrap_or_else(|_| app_data.join("Downloads"));
            let downloads = Arc::new(
                DownloadManager::new(Arc::clone(&db), download_dir)
                    .expect("Помилка ініціалізації менеджера завантажень"),
            );

            let webview_profile_dir = app
                .path()
                .app_local_data_dir()
                .unwrap_or_else(|_| app_data.clone())
                .join("webview2_profile");

            let webviews = Arc::new(WebviewManager::new(
                Arc::clone(&db),
                Arc::clone(&adblock),
                webview_profile_dir,
            ));

            if settings.restore_tabs_on_startup {
                let _ = webviews.restore_session_metadata();
            } else {
                let _ = webviews.init_startup_tab_metadata(settings.startup_page.clone());
            }

            // Запуск Memory Saver
            memory_saver::spawn_memory_saver(
                app.handle().clone(),
                Arc::clone(&db),
                Arc::clone(&webviews),
            );

            // Відкладене фонове завантаження кешу EasyList/EasyPrivacy (через 4с після старту вікна)
            let adblock_bg = Arc::clone(&adblock);
            let auto_update = settings.adblock_auto_update;
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                tracing::info!("AdBlock: фонове завантаження кешованих фільтрів");
                adblock_bg.load_cached_filters().await;
                if auto_update && adblock_bg.needs_filter_update() {
                    tracing::info!("AdBlock: фонове оновлення списків фільтрів");
                    let _ = adblock_bg.update_filter_lists().await;
                }
            });

            app.manage(AppState {
                db,
                adblock,
                passwords,
                downloads,
                webviews,
            });

            tracing::info!("Vortex setup: завершено успішно");
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::Resized(size) = event {
                    let size = *size;
                    if size.width > 0 && size.height > 0 {
                        let app_handle = window.app_handle().clone();
                        if let Some(state) = app_handle.try_state::<AppState>() {
                            if state.webviews.is_fullscreen_active() {
                                tauri::async_runtime::spawn(async move {
                                    if let Some(state) = app_handle.try_state::<AppState>() {
                                        state.webviews.sync_fullscreen_bounds(&app_handle, size);
                                    }
                                });
                            }
                        }
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_tabs,
            commands::get_active_tab_id,
            commands::create_tab,
            commands::switch_tab,
            commands::close_tab,
            commands::navigate_tab,
            commands::tab_nav_action,
            commands::set_viewport_bounds,
            commands::set_overlay_hidden,
            commands::get_adblock_status,
            commands::toggle_adblock_domain,
            commands::update_adblock_filters,
            commands::vault_status,
            commands::vault_setup,
            commands::vault_unlock,
            commands::vault_lock,
            commands::vault_list,
            commands::vault_save,
            commands::vault_delete,
            commands::vault_import_csv,
            commands::list_bookmarks,
            commands::save_bookmark,
            commands::delete_bookmark,
            commands::list_history,
            commands::delete_history_item,
            commands::clear_history_since,
            commands::list_downloads,
            commands::start_download,
            commands::pause_download,
            commands::resume_download,
            commands::open_download_folder,
            commands::get_settings,
            commands::update_settings,
            commands::get_process_memory_mb,
        ])
        .run(tauri::generate_context!())
        .expect("Критична помилка запуску Vortex Browser");
}
