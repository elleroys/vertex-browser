use crate::adblock::{AdBlockDomainStatus, AdBlockEngine};
use crate::db::{
    BookmarkFolder, BookmarkItem, BrowserSettings, Database, DownloadRecord, HistoryItem,
};
use crate::downloads::DownloadManager;
use crate::error::VortexResult;
use crate::password_manager::{PasswordCredential, PasswordManager, VaultStatus};
use crate::webview_manager::{BrowserTab, ViewportRect, WebviewManager};
use chrono::Utc;
use std::sync::Arc;
use tauri::{AppHandle, State};

pub struct AppState {
    pub db: Arc<Database>,
    pub adblock: Arc<AdBlockEngine>,
    pub passwords: Arc<PasswordManager>,
    pub downloads: Arc<DownloadManager>,
    pub webviews: Arc<WebviewManager>,
}

#[tauri::command]
pub async fn get_tabs(state: State<'_, AppState>) -> VortexResult<Vec<BrowserTab>> {
    Ok(state.webviews.list_tabs())
}

#[tauri::command]
pub async fn get_active_tab_id(state: State<'_, AppState>) -> VortexResult<Option<String>> {
    Ok(state.webviews.active_tab_id())
}

#[tauri::command]
pub async fn create_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    is_incognito: bool,
    activate: bool,
    tab_id: Option<String>,
) -> VortexResult<BrowserTab> {
    state
        .webviews
        .create_tab(&app, url, is_incognito, activate, tab_id)
}

#[tauri::command]
pub async fn switch_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: String,
) -> VortexResult<()> {
    state.webviews.switch_tab(&app, &tab_id)
}

#[tauri::command]
pub async fn close_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: String,
) -> VortexResult<()> {
    state.webviews.close_tab(&app, &tab_id)
}

#[tauri::command]
pub async fn navigate_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: String,
    url: String,
) -> VortexResult<()> {
    state.webviews.navigate_tab(&app, &tab_id, &url)
}

#[tauri::command]
pub async fn tab_nav_action(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: String,
    action: String,
) -> VortexResult<()> {
    state.webviews.eval_navigation_action(&app, &tab_id, &action)
}

#[tauri::command]
pub async fn set_viewport_bounds(
    app: AppHandle,
    state: State<'_, AppState>,
    rect: ViewportRect,
) -> VortexResult<()> {
    state.webviews.set_viewport(&app, rect)
}

#[tauri::command]
pub async fn set_overlay_hidden(
    app: AppHandle,
    state: State<'_, AppState>,
    hidden: bool,
) -> Result<(), String> {
    state.webviews.set_overlay_hidden(&app, hidden);
    Ok(())
}

// --- AdBlock ---

#[tauri::command]
pub async fn get_adblock_status(
    state: State<'_, AppState>,
    tab_id: String,
    page_url: String,
) -> VortexResult<AdBlockDomainStatus> {
    let adblock = Arc::clone(&state.adblock);
    tokio::task::spawn_blocking(move || adblock.get_status_for_url(&tab_id, &page_url))
        .await
        .map_err(|e| crate::error::VortexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
}

#[tauri::command]
pub async fn toggle_adblock_domain(
    state: State<'_, AppState>,
    domain: String,
    disable_on_domain: bool,
) -> VortexResult<bool> {
    let adblock = Arc::clone(&state.adblock);
    tokio::task::spawn_blocking(move || adblock.toggle_domain_whitelist(&domain, disable_on_domain))
        .await
        .map_err(|e| crate::error::VortexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
}

#[tauri::command]
pub async fn update_adblock_filters(state: State<'_, AppState>) -> VortexResult<usize> {
    state.adblock.update_filter_lists().await
}

// --- Паролі ---

#[tauri::command]
pub async fn vault_status(state: State<'_, AppState>) -> VortexResult<VaultStatus> {
    let passwords = Arc::clone(&state.passwords);
    tokio::task::spawn_blocking(move || passwords.status())
        .await
        .map_err(|e| crate::error::VortexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
}

#[tauri::command]
pub async fn vault_setup(
    state: State<'_, AppState>,
    master_password: String,
) -> VortexResult<()> {
    let passwords = Arc::clone(&state.passwords);
    tokio::task::spawn_blocking(move || passwords.setup_master_password(&master_password))
        .await
        .map_err(|e| crate::error::VortexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
}

#[tauri::command]
pub async fn vault_unlock(
    state: State<'_, AppState>,
    master_password: String,
) -> VortexResult<()> {
    let passwords = Arc::clone(&state.passwords);
    tokio::task::spawn_blocking(move || passwords.unlock(&master_password))
        .await
        .map_err(|e| crate::error::VortexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
}

#[tauri::command]
pub async fn vault_lock(state: State<'_, AppState>) -> VortexResult<()> {
    state.passwords.lock();
    Ok(())
}

#[tauri::command]
pub async fn vault_list(
    state: State<'_, AppState>,
    search: Option<String>,
) -> VortexResult<Vec<PasswordCredential>> {
    let passwords = Arc::clone(&state.passwords);
    tokio::task::spawn_blocking(move || passwords.list_credentials(search.as_deref()))
        .await
        .map_err(|e| crate::error::VortexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
}

#[tauri::command]
pub async fn vault_save(
    state: State<'_, AppState>,
    id: Option<String>,
    origin: String,
    title: String,
    username: String,
    password: String,
    notes: Option<String>,
) -> VortexResult<PasswordCredential> {
    let passwords = Arc::clone(&state.passwords);
    tokio::task::spawn_blocking(move || {
        passwords.save_credential(id, origin, title, username, password, notes)
    })
    .await
    .map_err(|e| crate::error::VortexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
}

#[tauri::command]
pub async fn vault_delete(state: State<'_, AppState>, id: String) -> VortexResult<()> {
    let passwords = Arc::clone(&state.passwords);
    tokio::task::spawn_blocking(move || passwords.delete_credential(&id))
        .await
        .map_err(|e| crate::error::VortexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
}

#[tauri::command]
pub async fn vault_import_csv(
    state: State<'_, AppState>,
    csv_content: String,
) -> VortexResult<usize> {
    let passwords = Arc::clone(&state.passwords);
    tokio::task::spawn_blocking(move || passwords.import_google_csv(&csv_content))
        .await
        .map_err(|e| crate::error::VortexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
}

// --- Закладки ---

#[tauri::command]
pub async fn list_bookmarks(
    state: State<'_, AppState>,
) -> VortexResult<(Vec<BookmarkFolder>, Vec<BookmarkItem>)> {
    let db = Arc::clone(&state.db);
    tokio::task::spawn_blocking(move || Ok((db.list_bookmark_folders()?, db.list_bookmarks()?)))
        .await
        .map_err(|e| crate::error::VortexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
}

#[tauri::command]
pub async fn save_bookmark(
    state: State<'_, AppState>,
    bookmark: BookmarkItem,
) -> VortexResult<()> {
    let db = Arc::clone(&state.db);
    tokio::task::spawn_blocking(move || db.upsert_bookmark(&bookmark))
        .await
        .map_err(|e| crate::error::VortexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
}

#[tauri::command]
pub async fn delete_bookmark(state: State<'_, AppState>, id: String) -> VortexResult<()> {
    let db = Arc::clone(&state.db);
    tokio::task::spawn_blocking(move || db.delete_bookmark(&id))
        .await
        .map_err(|e| crate::error::VortexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
}

// --- Історія ---

#[tauri::command]
pub async fn list_history(
    state: State<'_, AppState>,
    query: Option<String>,
    limit: Option<i64>,
) -> VortexResult<Vec<HistoryItem>> {
    let db = Arc::clone(&state.db);
    tokio::task::spawn_blocking(move || db.list_history(query.as_deref(), limit.unwrap_or(200)))
        .await
        .map_err(|e| crate::error::VortexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
}

#[tauri::command]
pub async fn delete_history_item(state: State<'_, AppState>, id: String) -> VortexResult<()> {
    let db = Arc::clone(&state.db);
    tokio::task::spawn_blocking(move || db.delete_history_item(&id))
        .await
        .map_err(|e| crate::error::VortexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
}

#[tauri::command]
pub async fn clear_history_since(
    state: State<'_, AppState>,
    hours_back: Option<i64>,
) -> VortexResult<()> {
    let db = Arc::clone(&state.db);
    let min_ts = match hours_back {
        Some(h) if h > 0 => Utc::now().timestamp_millis() - h * 3600 * 1000,
        _ => 0,
    };
    tokio::task::spawn_blocking(move || db.clear_history_since(min_ts))
        .await
        .map_err(|e| crate::error::VortexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
}

// --- Завантаження ---

#[tauri::command]
pub async fn list_downloads(state: State<'_, AppState>) -> VortexResult<Vec<DownloadRecord>> {
    let downloads = Arc::clone(&state.downloads);
    tokio::task::spawn_blocking(move || downloads.list())
        .await
        .map_err(|e| crate::error::VortexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
}

#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    filename: Option<String>,
    save_path: Option<String>,
) -> VortexResult<DownloadRecord> {
    state.downloads.start_download(app, url, filename, save_path)
}

#[tauri::command]
pub async fn pause_download(state: State<'_, AppState>, id: String) -> VortexResult<()> {
    state.downloads.pause(&id)
}

#[tauri::command]
pub async fn resume_download(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> VortexResult<DownloadRecord> {
    state.downloads.resume(app, &id)
}

#[tauri::command]
pub async fn open_download_folder(
    state: State<'_, AppState>,
    save_path: String,
) -> VortexResult<()> {
    state.downloads.open_folder(&save_path)
}

// --- Налаштування та телеметрія ---

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> VortexResult<BrowserSettings> {
    let db = Arc::clone(&state.db);
    tokio::task::spawn_blocking(move || db.get_settings())
        .await
        .map_err(|e| crate::error::VortexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    settings: BrowserSettings,
) -> VortexResult<()> {
    state.adblock.set_global_enabled(settings.adblock_enabled);
    let db = Arc::clone(&state.db);
    tokio::task::spawn_blocking(move || db.save_settings(&settings))
        .await
        .map_err(|e| crate::error::VortexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
}

#[tauri::command]
pub async fn get_process_memory_mb(state: State<'_, AppState>) -> VortexResult<f64> {
    Ok(state.webviews.measure_process_memory_mb())
}
