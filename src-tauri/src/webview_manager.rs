use crate::adblock::AdBlockEngine;
use crate::db::{Database, SessionTabRecord};
use crate::error::{VortexError, VortexResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{
    webview::{NewWindowResponse, PageLoadEvent},
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalPosition, PhysicalSize,
    Webview, WebviewBuilder, WebviewUrl,
};

pub const VORTEX_BROWSER_ARGS: &str = "--autoplay-policy=no-user-gesture-required --disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";
const VORTEX_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36 Edg/134.0.0.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewportRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTab {
    pub id: String,
    pub webview_label: String,
    pub url: String,
    pub title: String,
    pub favicon: Option<String>,
    pub is_loading: bool,
    pub is_https: bool,
    pub is_pinned: bool,
    pub is_incognito: bool,
    pub is_playing_audio: bool,
    pub is_muted: bool,
    pub is_suspended: bool,
    pub is_discarded: bool,
    pub has_active_download: bool,
    pub blocked_count: u64,
    pub last_active_at: i64,
    pub can_go_back: bool,
    pub can_go_forward: bool,
}

pub struct WebviewManager {
    tabs: Mutex<Vec<BrowserTab>>,
    active_tab_id: Mutex<Option<String>>,
    viewport: Mutex<ViewportRect>,
    overlay_hidden: AtomicBool,
    is_html_fullscreen: AtomicBool,
    was_window_fullscreen_before_html: AtomicBool,
    fullscreen_tab_id: Mutex<Option<String>>,
    spawn_lock: Mutex<()>,
    db: Arc<Database>,
    adblock: Arc<AdBlockEngine>,
    webview_data_dir: PathBuf,
}

impl WebviewManager {
    pub fn new(db: Arc<Database>, adblock: Arc<AdBlockEngine>, webview_data_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&webview_data_dir);
        Self {
            tabs: Mutex::new(Vec::new()),
            active_tab_id: Mutex::new(None),
            viewport: Mutex::new(ViewportRect {
                x: 0.0,
                y: 112.0,
                width: 1440.0,
                height: 788.0,
            }),
            overlay_hidden: AtomicBool::new(false),
            is_html_fullscreen: AtomicBool::new(false),
            was_window_fullscreen_before_html: AtomicBool::new(false),
            fullscreen_tab_id: Mutex::new(None),
            spawn_lock: Mutex::new(()),
            db,
            adblock,
            webview_data_dir,
        }
    }

    pub fn list_tabs(&self) -> Vec<BrowserTab> {
        self.tabs.lock().unwrap().clone()
    }

    pub fn active_tab_id(&self) -> Option<String> {
        self.active_tab_id.lock().unwrap().clone()
    }

    fn get_webview_by_tab_id(&self, app: &AppHandle, tab_id: &str) -> Option<Webview> {
        let label = self
            .tabs
            .lock()
            .unwrap()
            .iter()
            .find(|t| t.id == tab_id)
            .map(|t| t.webview_label.clone())?;
        app.get_webview(&label)
    }

    fn apply_fullscreen_bounds(app: &AppHandle, wv: &Webview) {
        if let Some(main_window) = app.get_window("main") {
            if let Ok(size) = main_window.inner_size() {
                if size.width > 0 && size.height > 0 {
                    let _ = wv.set_position(PhysicalPosition::new(0i32, 0i32));
                    let _ = wv.set_size(PhysicalSize::new(size.width, size.height));
                }
            }
        }
    }

    pub fn handle_fullscreen_changed(&self, app: &AppHandle, tab_id: &str, entering: bool) {
        tracing::info!(
            "handle_fullscreen_changed: tab_id={} entering={}",
            tab_id,
            entering
        );
        if entering {
            if self.active_tab_id().as_deref() != Some(tab_id) {
                if let Some(wv) = self.get_webview_by_tab_id(app, tab_id) {
                    let _ = wv.eval(
                        "if (document.fullscreenElement) { document.exitFullscreen().catch(() => {}); }",
                    );
                }
                return;
            }

            let already_fs = self.is_html_fullscreen.load(Ordering::Acquire);
            if !already_fs {
                let prev_win_fs = app
                    .get_window("main")
                    .and_then(|w| w.is_fullscreen().ok())
                    .unwrap_or(false);
                self.was_window_fullscreen_before_html
                    .store(prev_win_fs, Ordering::Release);
            }

            self.is_html_fullscreen.store(true, Ordering::Release);
            *self.fullscreen_tab_id.lock().unwrap() = Some(tab_id.to_string());

            if let Some(main_window) = app.get_window("main") {
                let _ = main_window.set_fullscreen(true);
            }
            let _ = app.emit("vortex://fullscreen-changed", true);

            if let Some(wv) = self.get_webview_by_tab_id(app, tab_id) {
                Self::apply_fullscreen_bounds(app, &wv);
                if !self.overlay_hidden.load(Ordering::Acquire) {
                    let _ = wv.show();
                    let _ = wv.set_focus();
                }
            }
        } else {
            self.exit_html_fullscreen(app, Some(tab_id));
        }
    }

    pub fn exit_html_fullscreen(&self, app: &AppHandle, target_tab_id: Option<&str>) {
        if !self.is_html_fullscreen.load(Ordering::Acquire) {
            return;
        }
        if let Some(tid) = target_tab_id {
            let current_fs_tab = self.fullscreen_tab_id.lock().unwrap().clone();
            if current_fs_tab.as_deref() != Some(tid) {
                return;
            }
        }

        tracing::info!("exit_html_fullscreen: target_tab_id={:?}", target_tab_id);
        self.is_html_fullscreen.store(false, Ordering::Release);
        *self.fullscreen_tab_id.lock().unwrap() = None;
        let prev_win_fs = self
            .was_window_fullscreen_before_html
            .swap(false, Ordering::AcqRel);

        if let Some(main_window) = app.get_window("main") {
            if !prev_win_fs {
                let _ = main_window.set_fullscreen(false);
            }
        }
        let _ = app.emit("vortex://fullscreen-changed", false);

        if !self.overlay_hidden.load(Ordering::Acquire) {
            if let Some(active_id) = self.active_tab_id() {
                let tab_opt = self
                    .tabs
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|t| t.id == active_id)
                    .cloned();
                if let Some(tab) = tab_opt {
                    if !is_internal_url(&tab.url) && !tab.is_suspended && !tab.is_discarded {
                        if let Some(wv) = app.get_webview(&tab.webview_label) {
                            let rect = self.viewport.lock().unwrap().clone();
                            let _ = wv.set_position(LogicalPosition::new(rect.x, rect.y));
                            let _ = wv.set_size(LogicalSize::new(rect.width, rect.height));
                            let _ = wv.show();
                        }
                    }
                }
            }
        }
    }

    pub fn is_fullscreen_active(&self) -> bool {
        self.is_html_fullscreen.load(Ordering::Acquire)
    }

    pub fn sync_fullscreen_bounds(&self, app: &AppHandle, size: PhysicalSize<u32>) {
        if !self.is_html_fullscreen.load(Ordering::Acquire) || size.width == 0 || size.height == 0 {
            return;
        }
        if self.overlay_hidden.load(Ordering::Acquire) {
            return;
        }
        let fs_tab_id = self
            .fullscreen_tab_id
            .lock()
            .unwrap()
            .clone()
            .or_else(|| self.active_tab_id());
        if let Some(tab_id) = fs_tab_id {
            if let Some(wv) = self.get_webview_by_tab_id(app, &tab_id) {
                let _ = wv.set_position(PhysicalPosition::new(0i32, 0i32));
                let _ = wv.set_size(PhysicalSize::new(size.width, size.height));
                let _ = wv.show();
            }
        }
    }

    pub fn set_viewport(&self, app: &AppHandle, rect: ViewportRect) -> VortexResult<()> {
        let in_html_fs = self.is_html_fullscreen.load(Ordering::Acquire);
        if !in_html_fs {
            *self.viewport.lock().unwrap() = rect.clone();
        }
        if let Some(active_id) = self.active_tab_id() {
            let tab_opt = self
                .tabs
                .lock()
                .unwrap()
                .iter()
                .find(|t| t.id == active_id)
                .cloned();
            if let Some(tab) = tab_opt {
                let overlay_hidden = self.overlay_hidden.load(Ordering::Acquire);
                if !is_internal_url(&tab.url) && !tab.is_suspended && !tab.is_discarded {
                    if let Some(wv) = app.get_webview(&tab.webview_label) {
                        if in_html_fs {
                            Self::apply_fullscreen_bounds(app, &wv);
                        } else {
                            let _ = wv.set_position(LogicalPosition::new(rect.x, rect.y));
                            let _ = wv.set_size(LogicalSize::new(rect.width, rect.height));
                        }
                        if !overlay_hidden {
                            let _ = wv.show();
                        }
                    } else if !overlay_hidden {
                        let wv = self.spawn_child_webview(app, &tab)?;
                        if in_html_fs {
                            Self::apply_fullscreen_bounds(app, &wv);
                        } else {
                            let _ = wv.set_position(LogicalPosition::new(rect.x, rect.y));
                            let _ = wv.set_size(LogicalSize::new(rect.width, rect.height));
                        }
                        let _ = wv.show();
                    }
                }
            }
        }
        Ok(())
    }

    pub fn set_overlay_hidden(&self, app: &AppHandle, hidden: bool) {
        self.overlay_hidden.store(hidden, Ordering::Release);
        let in_html_fs = self.is_html_fullscreen.load(Ordering::Acquire);
        if let Some(active_id) = self.active_tab_id() {
            let tab_opt = self
                .tabs
                .lock()
                .unwrap()
                .iter()
                .find(|t| t.id == active_id)
                .cloned();
            if let Some(tab) = tab_opt {
                if hidden || is_internal_url(&tab.url) || tab.is_suspended || tab.is_discarded {
                    if let Some(wv) = app.get_webview(&tab.webview_label) {
                        let _ = wv.hide();
                    }
                } else if let Some(wv) = app.get_webview(&tab.webview_label) {
                    if in_html_fs {
                        Self::apply_fullscreen_bounds(app, &wv);
                    } else {
                        let rect = self.viewport.lock().unwrap().clone();
                        let _ = wv.set_position(LogicalPosition::new(rect.x, rect.y));
                        let _ = wv.set_size(LogicalSize::new(rect.width, rect.height));
                    }
                    let _ = wv.show();
                }
            }
        }
    }

    pub fn create_tab(
        &self,
        app: &AppHandle,
        url: String,
        is_incognito: bool,
        activate: bool,
        custom_id: Option<String>,
    ) -> VortexResult<BrowserTab> {
        let id = custom_id
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        if let Some(existing) = self
            .tabs
            .lock()
            .unwrap()
            .iter()
            .find(|t| t.id == id)
            .cloned()
        {
            if activate {
                self.switch_tab(app, &existing.id)?;
            }
            return Ok(existing);
        }

        let short_id: String = id.chars().filter(|c| c.is_alphanumeric()).take(12).collect();
        let label = format!("tab-{}", short_id);
        let resolved_url = if url.trim().is_empty() {
            "vortex://newtab".to_string()
        } else {
            self.normalize_navigation_target(&url)
        };

        let tab = BrowserTab {
            id: id.clone(),
            webview_label: label.clone(),
            url: resolved_url.clone(),
            title: internal_title_for_url(&resolved_url),
            favicon: None,
            is_loading: !is_internal_url(&resolved_url),
            is_https: resolved_url.starts_with("https://"),
            is_pinned: false,
            is_incognito,
            is_playing_audio: false,
            is_muted: false,
            is_suspended: false,
            is_discarded: false,
            has_active_download: false,
            blocked_count: 0,
            last_active_at: Utc::now().timestamp_millis(),
            can_go_back: false,
            can_go_forward: false,
        };

        self.tabs.lock().unwrap().push(tab.clone());

        if !is_internal_url(&resolved_url) {
            self.spawn_child_webview(app, &tab)?;
        }

        if activate {
            self.switch_tab(app, &id)?;
        } else {
            self.persist_session();
        }

        Ok(tab)
    }

    pub fn switch_tab(&self, app: &AppHandle, tab_id: &str) -> VortexResult<()> {
        let prev_fs_tab = self.fullscreen_tab_id.lock().unwrap().clone();
        if self.is_html_fullscreen.load(Ordering::Acquire) && prev_fs_tab.as_deref() != Some(tab_id) {
            if let Some(prev_id) = prev_fs_tab {
                if let Some(prev_wv) = self.get_webview_by_tab_id(app, &prev_id) {
                    let _ = prev_wv.eval(
                        "if (document.fullscreenElement) { document.exitFullscreen().catch(() => {}); }",
                    );
                }
            }
            self.exit_html_fullscreen(app, None);
        }

        let now = Utc::now().timestamp_millis();
        let mut target_tab: Option<BrowserTab> = None;
        let mut was_discarded = false;
        let mut labels_to_hide: Vec<String> = Vec::new();

        {
            let mut tabs = self.tabs.lock().unwrap();
            for tab in tabs.iter_mut() {
                if tab.id == tab_id {
                    tab.last_active_at = now;
                    tab.is_suspended = false;
                    was_discarded = tab.is_discarded;
                    tab.is_discarded = false;
                    target_tab = Some(tab.clone());
                } else {
                    labels_to_hide.push(tab.webview_label.clone());
                }
            }
        }

        for label in labels_to_hide {
            if let Some(wv) = app.get_webview(&label) {
                let _ = wv.hide();
            }
        }

        let Some(tab) = target_tab else {
            return Err(VortexError::TabNotFound(tab_id.to_string()));
        };

        *self.active_tab_id.lock().unwrap() = Some(tab_id.to_string());

        if !is_internal_url(&tab.url) {
            if was_discarded || app.get_webview(&tab.webview_label).is_none() {
                let _ = self.spawn_child_webview(app, &tab)?;
            }
            if !self.overlay_hidden.load(Ordering::Acquire) {
                if let Some(wv) = app.get_webview(&tab.webview_label) {
                    if self.is_html_fullscreen.load(Ordering::Acquire) {
                        Self::apply_fullscreen_bounds(app, &wv);
                    } else {
                        let rect = self.viewport.lock().unwrap().clone();
                        let _ = wv.set_position(LogicalPosition::new(rect.x, rect.y));
                        let _ = wv.set_size(LogicalSize::new(rect.width, rect.height));
                    }
                    let _ = wv.show();
                    let _ = wv.set_focus();
                }
            }
        } else if let Some(wv) = app.get_webview(&tab.webview_label) {
            let _ = wv.hide();
        }

        self.persist_session();
        Ok(())
    }

    pub fn navigate_tab(&self, app: &AppHandle, tab_id: &str, raw_url: &str) -> VortexResult<()> {
        let url_str = self.normalize_navigation_target(raw_url);
        let mut target_tab: Option<BrowserTab> = None;

        {
            let mut tabs = self.tabs.lock().unwrap();
            if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                tab.url = url_str.clone();
                if is_internal_url(&url_str) {
                    tab.title = internal_title_for_url(&url_str);
                }
                tab.is_https = url_str.starts_with("https://");
                tab.is_loading = !is_internal_url(&url_str);
                tab.is_suspended = false;
                tab.is_discarded = false;
                tab.last_active_at = Utc::now().timestamp_millis();
                target_tab = Some(tab.clone());
            }
        }

        let tab = match target_tab {
            Some(t) => t,
            None => {
                // Якщо вкладка з таким ID ще не синхронізована з фронтенду — створюємо її автоматично
                return self
                    .create_tab(app, url_str, false, true, Some(tab_id.to_string()))
                    .map(|_| ());
            }
        };

        if self.active_tab_id().is_none() {
            *self.active_tab_id.lock().unwrap() = Some(tab.id.clone());
        }
        let is_active = self.active_tab_id().as_deref() == Some(tab_id);

        self.adblock.reset_tab_counter(&tab.id);

        if is_internal_url(&url_str) {
            if self.fullscreen_tab_id.lock().unwrap().as_deref() == Some(tab_id) {
                self.exit_html_fullscreen(app, Some(tab_id));
            }
            if let Some(wv) = app.get_webview(&tab.webview_label) {
                let _ = wv.hide();
            }
        } else if let Some(wv) = app.get_webview(&tab.webview_label) {
            let parsed =
                url::Url::parse(&url_str).map_err(|e| VortexError::InvalidUrl(e.to_string()))?;
            wv.navigate(parsed)
                .map_err(|e| VortexError::Webview(e.to_string()))?;
            if is_active && !self.overlay_hidden.load(Ordering::Acquire) {
                if self.is_html_fullscreen.load(Ordering::Acquire) {
                    Self::apply_fullscreen_bounds(app, &wv);
                } else {
                    let rect = self.viewport.lock().unwrap().clone();
                    let _ = wv.set_position(LogicalPosition::new(rect.x, rect.y));
                    let _ = wv.set_size(LogicalSize::new(rect.width, rect.height));
                }
                let _ = wv.show();
                let _ = wv.set_focus();
            }
        } else {
            let wv = self.spawn_child_webview(app, &tab)?;
            if is_active && !self.overlay_hidden.load(Ordering::Acquire) {
                if self.is_html_fullscreen.load(Ordering::Acquire) {
                    Self::apply_fullscreen_bounds(app, &wv);
                } else {
                    let rect = self.viewport.lock().unwrap().clone();
                    let _ = wv.set_position(LogicalPosition::new(rect.x, rect.y));
                    let _ = wv.set_size(LogicalSize::new(rect.width, rect.height));
                }
                let _ = wv.show();
                let _ = wv.set_focus();
            }
        }

        if !tab.is_incognito && !is_internal_url(&url_str) {
            let _ = self.db.record_history(&tab.title, &url_str, None);
        }

        let _ = app.emit("vortex://tab-updated", &tab);
        self.persist_session();
        Ok(())
    }

    pub fn close_tab(&self, app: &AppHandle, tab_id: &str) -> VortexResult<()> {
        if self.fullscreen_tab_id.lock().unwrap().as_deref() == Some(tab_id) {
            self.exit_html_fullscreen(app, Some(tab_id));
        }

        let removed;
        let next_active;
        let cur_active = self.active_tab_id();
        {
            let mut tabs = self.tabs.lock().unwrap();
            let Some(pos) = tabs.iter().position(|t| t.id == tab_id) else {
                return Ok(());
            };
            removed = tabs.remove(pos);

            next_active = if cur_active.as_deref() == Some(tab_id) {
                if tabs.is_empty() {
                    None
                } else {
                    let idx = pos.min(tabs.len() - 1);
                    Some(tabs[idx].id.clone())
                }
            } else {
                cur_active
            };
        }

        if let Some(wv) = app.get_webview(&removed.webview_label) {
            let _ = wv.close();
        }

        if let Some(next_id) = next_active {
            self.switch_tab(app, &next_id)?;
        } else {
            *self.active_tab_id.lock().unwrap() = None;
        }

        self.persist_session();
        Ok(())
    }

    pub fn eval_navigation_action(&self, app: &AppHandle, tab_id: &str, action: &str) -> VortexResult<()> {
        let webview_label = {
            let tabs = self.tabs.lock().unwrap();
            tabs.iter()
                .find(|t| t.id == tab_id)
                .map(|t| t.webview_label.clone())
                .ok_or_else(|| VortexError::TabNotFound(tab_id.to_string()))?
        };
        if let Some(wv) = app.get_webview(&webview_label) {
            let js = match action {
                "back" => "window.history.back();",
                "forward" => "window.history.forward();",
                "reload" => "window.location.reload();",
                "stop" => "window.stop();",
                _ => "",
            };
            if !js.is_empty() {
                wv.eval(js).map_err(|e| VortexError::Webview(e.to_string()))?;
            }
        }
        Ok(())
    }

    pub fn handle_page_load(
        &self,
        app: &AppHandle,
        tab_id: &str,
        event: PageLoadEvent,
        url_str: &str,
    ) {
        if url_str.is_empty() || url_str == "about:blank" {
            return;
        }
        let mut updated: Option<BrowserTab> = None;
        {
            let mut tabs = self.tabs.lock().unwrap();
            if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                if !is_internal_url(&tab.url) {
                    tab.url = url_str.to_string();
                    tab.is_https = url_str.starts_with("https://");
                    tab.is_loading = matches!(event, PageLoadEvent::Started);
                    tab.last_active_at = Utc::now().timestamp_millis();
                    updated = Some(tab.clone());
                }
            }
        }

        if let Some(tab) = updated {
            if matches!(event, PageLoadEvent::Finished)
                && !tab.is_incognito
                && !is_internal_url(&tab.url)
            {
                let _ = self.db.record_history(&tab.title, &tab.url, None);
            }
            let _ = app.emit("vortex://tab-updated", &tab);
            if matches!(event, PageLoadEvent::Finished) {
                self.persist_session();
            }
        }
    }

    pub fn handle_title_changed(&self, app: &AppHandle, tab_id: &str, title: String) {
        let trimmed = title.trim().to_string();
        if trimmed.is_empty() {
            return;
        }
        let mut updated: Option<BrowserTab> = None;
        {
            let mut tabs = self.tabs.lock().unwrap();
            if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                if !is_internal_url(&tab.url) {
                    tab.title = trimmed.clone();
                    updated = Some(tab.clone());
                }
            }
        }

        if let Some(tab) = updated {
            if !tab.is_incognito && !is_internal_url(&tab.url) {
                let _ = self.db.record_history(&tab.title, &tab.url, None);
            }
            let _ = app.emit("vortex://tab-updated", &tab);
            self.persist_session();
        }
    }

    /// Memory Saver: призупиняє (`TrySuspend`) або вивантажує (`Discard`) неактивні вкладки.
    pub fn apply_memory_saver(
        &self,
        app: &AppHandle,
        suspend_after_secs: i64,
        discard_after_secs: i64,
    ) -> bool {
        let now = Utc::now().timestamp_millis();
        let active_id = self.active_tab_id();
        let mut changed = false;
        let mut to_discard: Vec<String> = Vec::new();
        let mut to_suspend: Vec<String> = Vec::new();

        {
            let mut tabs = self.tabs.lock().unwrap();
            for tab in tabs.iter_mut() {
                if Some(tab.id.as_str()) == active_id.as_deref()
                    || tab.is_pinned
                    || tab.is_playing_audio
                    || tab.has_active_download
                    || is_internal_url(&tab.url)
                {
                    continue;
                }

                let idle_secs = (now - tab.last_active_at) / 1000;

                if idle_secs >= discard_after_secs && !tab.is_discarded {
                    to_discard.push(tab.webview_label.clone());
                    tab.is_discarded = true;
                    tab.is_suspended = false;
                    changed = true;
                } else if idle_secs >= suspend_after_secs
                    && !tab.is_suspended
                    && !tab.is_discarded
                {
                    to_suspend.push(tab.webview_label.clone());
                    tab.is_suspended = true;
                    changed = true;
                }
            }
        }

        for label in to_discard {
            if let Some(wv) = app.get_webview(&label) {
                let _ = wv.close();
            }
        }

        #[cfg(target_os = "windows")]
        for label in to_suspend {
            if let Some(wv) = app.get_webview(&label) {
                let _ = wv.with_webview(|platform_wv| unsafe {
                    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_3;
                    use windows::core::Interface;
                    if let Ok(wv3) = platform_wv
                        .controller()
                        .CoreWebView2()
                        .and_then(|c| c.cast::<ICoreWebView2_3>())
                    {
                        let _ = wv3.TrySuspend(None);
                    }
                });
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = to_suspend;

        changed
    }

    /// Створює справжній дочірній WebView2 всередині головного вікна Tauri 2 (без iframe!).
    fn spawn_child_webview(&self, app: &AppHandle, tab: &BrowserTab) -> VortexResult<Webview> {
        tracing::info!("spawn_child_webview: {} ({})", tab.id, tab.url);
        let _spawn_guard = self.spawn_lock.lock().unwrap();
        if let Some(existing) = app.get_webview(&tab.webview_label) {
            return Ok(existing);
        }

        let main_window = app
            .get_window("main")
            .ok_or_else(|| VortexError::Webview("Головне вікно не знайдено".into()))?;

        let parsed = url::Url::parse(&tab.url)
            .map_err(|e| VortexError::InvalidUrl(e.to_string()))?;
        let rect = self.viewport.lock().unwrap().clone();

        let app_for_load = app.clone();
        let tab_id_for_load = tab.id.clone();

        let app_for_title = app.clone();
        let tab_id_for_title = tab.id.clone();

        let app_for_popup = app.clone();

        let mut builder = WebviewBuilder::new(&tab.webview_label, WebviewUrl::External(parsed))
            .incognito(tab.is_incognito)
            .data_directory(self.webview_data_dir.clone())
            .user_agent(VORTEX_USER_AGENT)
            .on_page_load(move |_wv, payload| {
                if let Some(state) = app_for_load.try_state::<crate::commands::AppState>() {
                    state.webviews.handle_page_load(
                        &app_for_load,
                        &tab_id_for_load,
                        payload.event(),
                        payload.url().as_str(),
                    );
                }
            })
            .on_document_title_changed(move |_wv, title| {
                if let Some(state) = app_for_title.try_state::<crate::commands::AppState>() {
                    state.webviews.handle_title_changed(
                        &app_for_title,
                        &tab_id_for_title,
                        title,
                    );
                }
            })
            .on_new_window(move |url, _features| {
                let _ = app_for_popup.emit("vortex://open-new-tab", url.to_string());
                NewWindowResponse::Deny
            });

        #[cfg(target_os = "windows")]
        {
            builder = builder.additional_browser_args(VORTEX_BROWSER_ARGS);
        }

        let webview = main_window
            .add_child(
                builder,
                LogicalPosition::new(rect.x, rect.y),
                LogicalSize::new(rect.width, rect.height),
            )
            .map_err(|e| VortexError::Webview(e.to_string()))?;

        // На Windows 11 налаштовуємо ICoreWebView2Settings та безпечний перехоплювач підресурсів для AdBlock
        #[cfg(target_os = "windows")]
        {
            let adblock = Arc::clone(&self.adblock);
            let tab_id = tab.id.clone();
            let fallback_page_url = tab.url.clone();
            let app_handle = app.clone();

            let _ = webview.with_webview(move |platform_wv| unsafe {
                use webview2_com::Microsoft::Web::WebView2::Win32::{
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE,
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_MEDIA,
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_OTHER,
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_PING,
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SCRIPT,
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_STYLESHEET,
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST,
                };
                use webview2_com::{
                    ContainsFullScreenElementChangedEventHandler, WebResourceRequestedEventHandler,
                };
                use windows::core::{BOOL, PWSTR};
                use windows::Win32::System::Com::{CoTaskMemFree, IStream};

                let env = platform_wv.environment();
                if let Ok(core) = platform_wv.controller().CoreWebView2() {
                    if let Ok(settings) = core.Settings() {
                        let _ = settings.SetIsScriptEnabled(true.into());
                        let _ = settings.SetIsWebMessageEnabled(true.into());
                        let _ = settings.SetAreDefaultScriptDialogsEnabled(true.into());
                        let _ = settings.SetAreDefaultContextMenusEnabled(true.into());
                        let _ = settings.SetIsZoomControlEnabled(true.into());
                        let _ = settings.SetIsBuiltInErrorPageEnabled(true.into());
                        let _ = settings.SetIsStatusBarEnabled(false.into());
                    }

                    // Повноекранний режим для YouTube та інших HTML5 плеєрів
                    let core_for_fs = core.clone();
                    let app_for_fs = app_handle.clone();
                    let tab_id_for_fs = tab_id.clone();
                    let fs_handler = ContainsFullScreenElementChangedEventHandler::create(Box::new(
                        move |_sender, _args| {
                            let mut contains_fs = BOOL::default();
                            if core_for_fs.ContainsFullScreenElement(&mut contains_fs).is_ok() {
                                let entering = contains_fs.as_bool();
                                let app_inner = app_for_fs.clone();
                                let tab_inner = tab_id_for_fs.clone();
                                tauri::async_runtime::spawn(async move {
                                    if let Some(state) =
                                        app_inner.try_state::<crate::commands::AppState>()
                                    {
                                        state.webviews.handle_fullscreen_changed(
                                            &app_inner,
                                            &tab_inner,
                                            entering,
                                        );
                                    }
                                });
                            }
                            Ok(())
                        },
                    ));
                    let mut fs_token = 0i64;
                    let _ = core.add_ContainsFullScreenElementChanged(&fs_handler, &mut fs_token);

                    // Перехоплюємо ЛИШЕ HTTP/HTTPS підресурси (без DOCUMENT, blob:, data: та workers)
                    for ctx in [
                        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SCRIPT,
                        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE,
                        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_STYLESHEET,
                        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST,
                        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_MEDIA,
                        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_PING,
                        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_OTHER,
                    ] {
                        let _ = core.AddWebResourceRequestedFilter(windows::core::w!("https://*"), ctx);
                        let _ = core.AddWebResourceRequestedFilter(windows::core::w!("http://*"), ctx);
                    }

                    let core_for_handler = core.clone();
                    let handler = WebResourceRequestedEventHandler::create(Box::new(
                        move |_sender, args| {
                            if let Some(args) = args {
                                if let Ok(req) = args.Request() {
                                    let mut uri_ptr = PWSTR::null();
                                    if req.Uri(&mut uri_ptr).is_ok() && !uri_ptr.is_null() {
                                        let req_url = uri_ptr.to_string().unwrap_or_default();
                                        CoTaskMemFree(Some(uri_ptr.as_ptr() as _));

                                        let mut src_ptr = PWSTR::null();
                                        let live_page_url = if core_for_handler.Source(&mut src_ptr).is_ok()
                                            && !src_ptr.is_null()
                                        {
                                            let s = src_ptr.to_string().unwrap_or_default();
                                            CoTaskMemFree(Some(src_ptr.as_ptr() as _));
                                            if s.is_empty() || s == "about:blank" {
                                                fallback_page_url.clone()
                                            } else {
                                                s
                                            }
                                        } else {
                                            fallback_page_url.clone()
                                        };

                                        let blocked = adblock.should_block_request(
                                            &tab_id,
                                            &req_url,
                                            &live_page_url,
                                            "other",
                                        );
                                        if blocked {
                                            if let Ok(resp) = env.CreateWebResourceResponse(
                                                None::<&IStream>,
                                                403,
                                                windows::core::w!("Blocked by Vortex AdBlock"),
                                                windows::core::w!("Content-Type: text/plain"),
                                            ) {
                                                let _ = args.SetResponse(&resp);
                                                let _ = app_handle.emit("vortex://adblock-hit", &tab_id);
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(())
                        },
                    ));
                    let mut token = 0i64;
                    let _ = core.add_WebResourceRequested(&handler, &mut token);
                }
            });
        }

        let should_show = self.active_tab_id().as_deref() == Some(tab.id.as_str())
            && !self.overlay_hidden.load(Ordering::Acquire);
        if !should_show {
            let _ = webview.hide();
        }
        Ok(webview)
    }

    /// Ініціалізує метадані вкладок при запуску без синхронного виклику `add_child` у `setup()`.
    /// Дочірній WebView2 для активної зовнішньої вкладки створюється асинхронно при першому `set_viewport` / `switch_tab`.
    pub fn restore_session_metadata(&self) -> VortexResult<Vec<BrowserTab>> {
        let saved = self.db.load_session_tabs()?;
        if saved.is_empty() {
            return self.init_startup_tab_metadata("vortex://newtab".to_string());
        }

        let now = Utc::now().timestamp_millis();
        let mut active_id: Option<String> = None;
        let mut restored = Vec::with_capacity(saved.len());

        for rec in saved {
            let short_id: String = rec
                .id
                .chars()
                .filter(|c| c.is_alphanumeric())
                .take(12)
                .collect();
            let resolved_url = if rec.url.trim().is_empty() {
                "vortex://newtab".to_string()
            } else {
                rec.url
            };
            if rec.is_active {
                active_id = Some(rec.id.clone());
            }
            restored.push(BrowserTab {
                id: rec.id,
                webview_label: format!("tab-{}", short_id),
                url: resolved_url.clone(),
                title: if rec.title.trim().is_empty() {
                    internal_title_for_url(&resolved_url)
                } else {
                    rec.title
                },
                favicon: rec.favicon,
                is_loading: false,
                is_https: resolved_url.starts_with("https://"),
                is_pinned: rec.is_pinned,
                is_incognito: false,
                is_playing_audio: false,
                is_muted: false,
                is_suspended: false,
                is_discarded: false,
                has_active_download: false,
                blocked_count: 0,
                last_active_at: now,
                can_go_back: false,
                can_go_forward: false,
            });
        }

        if active_id.is_none() {
            active_id = restored.first().map(|t| t.id.clone());
        }

        *self.tabs.lock().unwrap() = restored.clone();
        *self.active_tab_id.lock().unwrap() = active_id;
        Ok(restored)
    }

    pub fn init_startup_tab_metadata(&self, url: String) -> VortexResult<Vec<BrowserTab>> {
        let id = uuid::Uuid::new_v4().to_string();
        let short_id: String = id.chars().filter(|c| c.is_alphanumeric()).take(12).collect();
        let resolved_url = if url.trim().is_empty() {
            "vortex://newtab".to_string()
        } else {
            self.normalize_navigation_target(&url)
        };
        let tab = BrowserTab {
            id: id.clone(),
            webview_label: format!("tab-{}", short_id),
            url: resolved_url.clone(),
            title: internal_title_for_url(&resolved_url),
            favicon: None,
            is_loading: false,
            is_https: resolved_url.starts_with("https://"),
            is_pinned: false,
            is_incognito: false,
            is_playing_audio: false,
            is_muted: false,
            is_suspended: false,
            is_discarded: false,
            has_active_download: false,
            blocked_count: 0,
            last_active_at: Utc::now().timestamp_millis(),
            can_go_back: false,
            can_go_forward: false,
        };
        *self.tabs.lock().unwrap() = vec![tab.clone()];
        *self.active_tab_id.lock().unwrap() = Some(id);
        self.persist_session();
        Ok(vec![tab])
    }

    fn persist_session(&self) {
        let active_id = self.active_tab_id();
        let records: Vec<SessionTabRecord> = {
            let tabs = self.tabs.lock().unwrap();
            tabs.iter()
                .filter(|t| !t.is_incognito)
                .enumerate()
                .map(|(idx, t)| SessionTabRecord {
                    id: t.id.clone(),
                    url: t.url.clone(),
                    title: t.title.clone(),
                    favicon: t.favicon.clone(),
                    is_pinned: t.is_pinned,
                    is_active: Some(t.id.as_str()) == active_id.as_deref(),
                    sort_order: idx as i64,
                })
                .collect()
        };
        let _ = self.db.save_session_tabs(&records);
    }

    /// Вимірювання реального споживання RAM процесами Vortex + WebView2 (Windows Working Set).
    pub fn measure_process_memory_mb(&self) -> f64 {
        #[cfg(target_os = "windows")]
        unsafe {
            use windows::Win32::System::ProcessStatus::{
                K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
            };
            use windows::Win32::System::Threading::GetCurrentProcess;
            let mut pmc = PROCESS_MEMORY_COUNTERS::default();
            if K32GetProcessMemoryInfo(
                GetCurrentProcess(),
                &mut pmc,
                std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            )
            .as_bool()
            {
                return (pmc.WorkingSetSize as f64) / (1024.0 * 1024.0);
            }
        }
        0.0
    }

    fn normalize_navigation_target(&self, raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return "vortex://newtab".to_string();
        }
        if !trimmed.contains(char::is_whitespace)
            && (is_internal_url(trimmed)
                || trimmed.starts_with("http://")
                || trimmed.starts_with("https://"))
        {
            return trimmed.to_string();
        }
        if !trimmed.contains(char::is_whitespace) {
            let lower = trimmed.to_ascii_lowercase();
            if lower == "localhost"
                || lower.starts_with("localhost:")
                || lower.starts_with("localhost/")
            {
                return format!("http://{}", trimmed);
            }
            let host_part = trimmed
                .split(['/', '?', '#'])
                .next()
                .unwrap_or("")
                .split(':')
                .next()
                .unwrap_or("");
            let segments: Vec<&str> = host_part.split('.').collect();
            let valid_domain = segments.len() >= 2
                && segments
                    .iter()
                    .all(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
                && segments
                    .last()
                    .is_some_and(|tld| tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()));
            let valid_ipv4 = segments.len() == 4
                && segments.iter().all(|s| s.parse::<u8>().is_ok());

            if valid_domain {
                return format!("https://{}", trimmed);
            }
            if valid_ipv4 {
                return format!("http://{}", trimmed);
            }
        }
        let engine = self
            .db
            .get_settings()
            .map(|s| s.search_engine)
            .unwrap_or_else(|_| "google".to_string());
        let encoded: String = url::form_urlencoded::byte_serialize(trimmed.as_bytes()).collect();
        match engine.as_str() {
            "duckduckgo" => format!("https://duckduckgo.com/?q={}", encoded),
            "brave" => format!("https://search.brave.com/search?q={}", encoded),
            "bing" => format!("https://www.bing.com/search?q={}", encoded),
            _ => format!("https://www.google.com/search?q={}", encoded),
        }
    }
}

fn is_internal_url(url: &str) -> bool {
    url.starts_with("vortex://") || url.starts_with("about:")
}

fn internal_title_for_url(url: &str) -> String {
    match url {
        "vortex://newtab" => "Нова вкладка".to_string(),
        "vortex://settings" => "Налаштування".to_string(),
        "vortex://benchmarks" => "Тестування продуктивності".to_string(),
        _ => url.to_string(),
    }
}
