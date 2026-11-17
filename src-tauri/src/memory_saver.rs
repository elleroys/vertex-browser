use crate::db::Database;
use crate::webview_manager::WebviewManager;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Періодичний менеджер оптимізації оперативної пам'яті (Memory Saver).
/// Замість агресивного опитування використовує інтервал перевірки 30 секунд
/// і ніколи не вивантажує активні, закріплені вкладки, вкладки зі звуком або активні завантаження.
pub fn spawn_memory_saver(
    app: AppHandle,
    db: Arc<Database>,
    webviews: Arc<WebviewManager>,
) {
    tauri::async_runtime::spawn(async move {
        let start = tokio::time::Instant::now() + Duration::from_secs(30);
        let mut ticker = tokio::time::interval_at(start, Duration::from_secs(30));
        loop {
            ticker.tick().await;
            let Ok(settings) = db.get_settings() else {
                continue;
            };
            if settings.memory_saver_mode == "off" {
                continue;
            }

            let (suspend_secs, discard_secs) = match settings.memory_saver_mode.as_str() {
                "aggressive" => (120i64, 300i64),
                _ => (
                    settings.memory_saver_suspend_mins.max(1) * 60,
                    settings.memory_saver_discard_mins.max(2) * 60,
                ),
            };

            let changed = webviews.apply_memory_saver(&app, suspend_secs, discard_secs);
            if changed {
                let snapshot = webviews.list_tabs();
                let _ = app.emit("vortex://tabs-updated", snapshot);
            }
        }
    });
}
