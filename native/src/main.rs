mod adblock;
mod password_vault;
mod storage;

use std::{
    cell::{Cell, RefCell},
    error::Error,
    rc::Rc,
};

use servo::{
    Code, DevicePoint, EventLoopWaker, InputEvent, Key, KeyState, LoadStatus, Location, Modifiers,
    MouseButton as ServoMouseButton, MouseButtonAction, MouseButtonEvent,
    ImeEvent, MouseLeftViewportEvent, MouseMoveEvent, NamedKey, Servo, ServoBuilder,
    WebView, WebViewBuilder, WebViewPoint, WheelDelta, WheelEvent, WheelMode,
    WindowRenderingContext, OffscreenRenderingContext, RenderingContext, KeyboardEvent,
};
use url::Url;
use zeroize::{Zeroize, Zeroizing};
use storage::{default_profile_dir, BrowserStorage};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, Ime, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    keyboard::{Key as WinitKey, KeyCode, ModifiersState, NamedKey as WinitNamedKey, PhysicalKey},
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::{Window, WindowId},
};

fn main() -> Result<(), Box<dyn Error>> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    let event_loop = EventLoop::<ServoWakeup>::with_user_event().build()?;
    let mut app = VertexApp::new(&event_loop);
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct BrowserState {
    window: Window,
    servo: Servo,
    rendering_context: Rc<WindowRenderingContext>,
    page_context: Rc<OffscreenRenderingContext>,
    webviews: RefCell<Vec<WebView>>,
    active_tab: Cell<usize>,
    closed_tabs: RefCell<Vec<Url>>,
    cursor_position: Cell<PhysicalPosition<f64>>,
    modifiers: Cell<Modifiers>,
    storage: BrowserStorage,
    egui: RefCell<egui_glow::winit::EguiGlow>,
    address_bar: RefCell<String>,
    adblock_custom_rules: RefCell<String>,
    adblock_rules: RefCell<adblock::RuleSet>,
    adblock_rules_draft: RefCell<String>,
    adblock_rules_open: Cell<bool>,
    password_vault: password_vault::PasswordVault,
    password_vault_open: Cell<bool>,
    vault_master_password: RefCell<String>,
    vault_confirm_password: RefCell<String>,
    vault_origin: RefCell<String>,
    vault_title: RefCell<String>,
    vault_username: RefCell<String>,
    vault_password: RefCell<String>,
    vault_notes: RefCell<String>,
    vault_search: RefCell<String>,
    vault_show_passwords: Cell<bool>,
    vault_message: RefCell<String>,
}

impl BrowserState {
    fn active_webview(&self) -> Option<WebView> {
        let tabs = self.webviews.borrow();
        tabs.get(self.active_tab.get()).cloned()
    }

    fn tab_count(&self) -> usize {
        self.webviews.borrow().len()
    }

    fn activate_tab(&self, index: usize) {
        let mut tabs = self.webviews.borrow_mut();
        if index >= tabs.len() {
            return;
        }
        if let Some(previous) = tabs.get(self.active_tab.get()) {
            previous.set_focused(false);
            previous.hide();
        }
        self.active_tab.set(index);
        if let Some(active) = tabs.get(index) {
            active.show();
            active.set_focused(true);
            self.update_window_title(active);
            *self.address_bar.borrow_mut() = active.url().filter(|url| matches!(url.scheme(), "http" | "https")).map(|url| url.to_string()).unwrap_or_default();
        }
        self.window.request_redraw();
    }

    fn create_tab(self: &Rc<Self>, url: Url) {
        if let Some(active) = self.active_webview() {
            active.set_focused(false);
            active.hide();
        }
        let webview = WebViewBuilder::new(&self.servo, self.page_context.clone())
            .url(url)
            .hidpi_scale_factor(euclid::Scale::new(self.window.scale_factor() as f32))
            .delegate(self.clone())
            .build();
        let mut tabs = self.webviews.borrow_mut();
        tabs.push(webview);
        let index = tabs.len() - 1;
        self.active_tab.set(index);
        if let Some(active) = tabs.get(index) {
            active.show();
            active.set_focused(true);
            self.update_window_title(active);
            *self.address_bar.borrow_mut() = active.url()
                .filter(|url| matches!(url.scheme(), "http" | "https"))
                .map(|url| url.to_string()).unwrap_or_default();
        }
        drop(tabs);
        self.window.request_redraw();
    }

    fn close_active_tab(&self) {
        self.close_tab_at(self.active_tab.get());
    }

    fn reopen_closed_tab(self: &Rc<Self>) {
        let url = self.closed_tabs.borrow_mut().pop();
        if let Some(url) = url {
            self.create_tab(url);
        }
    }

    fn close_tab_at(&self, index: usize) {
        let mut tabs = self.webviews.borrow_mut();
        if tabs.len() <= 1 || index >= tabs.len() {
            return;
        }
        let was_active = index == self.active_tab.get();
        if let Some(url) = tabs[index].url().filter(|url| matches!(url.scheme(), "http" | "https")) {
            let mut closed = self.closed_tabs.borrow_mut();
            closed.push(url);
            if closed.len() > 10 { closed.remove(0); }
        }
        tabs[index].set_focused(false);
        tabs[index].hide();
        tabs.remove(index);

        let next = if index < self.active_tab.get() {
            self.active_tab.get() - 1
        } else if was_active {
            index.min(tabs.len() - 1)
        } else {
            self.active_tab.get()
        };
        self.active_tab.set(next);

        if let Some(active) = tabs.get(next) {
            active.show();
            active.set_focused(true);
            if was_active {
                self.update_window_title(active);
                *self.address_bar.borrow_mut() = active.url()
                    .filter(|url| matches!(url.scheme(), "http" | "https"))
                    .map(|url| url.to_string()).unwrap_or_default();
            }
        }
        drop(tabs);
        self.window.request_redraw();
    }

    fn update_window_title(&self, webview: &WebView) {
        let title = webview
            .page_title()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| webview.url().and_then(|url| url.host_str().map(str::to_owned)))
            .unwrap_or_else(|| "New tab".to_owned());
        self.window.set_title(&format!("{title} — Vertex Browser"));
    }

    fn open_internal_page(&self, html: String) {
        let url = html_data_url(&html);
        if let Some(view) = self.active_webview() {
            view.load(url);
            self.servo.spin_event_loop();
        }
    }

    fn new_tab_url(&self) -> Url {
        let bookmarks = self.storage.all_bookmarks().unwrap_or_default();
        let history = self.storage.recent_history(8).unwrap_or_default();
        let mut sections = String::new();

        sections.push_str("<section><h2>Швидкий доступ</h2><div class=\"tiles\">");
        for entry in bookmarks.iter().take(8) {
            let title = html_escape(if entry.title.trim().is_empty() { &entry.url } else { &entry.title });
            let href = html_escape(&entry.url);
            sections.push_str(&format!("<a class=\"tile\" href=\"{href}\">{title}</a>"));
        }
        if bookmarks.is_empty() {
            sections.push_str("<p class=\"muted\">Закріплені сайти з’являться тут після додавання закладок через Ctrl+D.</p>");
        }
        sections.push_str("</div></section>");

        sections.push_str("<section><h2>Нещодавно відвідані</h2>");
        for entry in history.iter() {
            let title = html_escape(if entry.title.trim().is_empty() { &entry.url } else { &entry.title });
            let href = html_escape(&entry.url);
            sections.push_str(&format!("<article><a href=\"{href}\">{title}</a><small>{}</small></article>", html_escape(&entry.url)));
        }
        if history.is_empty() {
            sections.push_str("<p class=\"muted\">Тут відображатимуться останні відвідані сторінки.</p>");
        }
        sections.push_str("</section>");

        let html = format!(r#"<!doctype html>
<html lang="uk"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Нова вкладка — Vertex</title><style>
:root{{color-scheme:dark;font-family:Segoe UI,Arial,sans-serif;background:#111318;color:#e8eaf0}}
*{{box-sizing:border-box}}body{{margin:0;padding:12vh max(24px,calc((100vw - 900px)/2));}}
.brand{{color:#9aa8ff;font-size:12px;letter-spacing:3px;font-weight:700;text-align:center}}
h1{{font-size:42px;text-align:center;margin:12px 0 34px;font-weight:650}}
form{{display:flex;gap:10px;margin:0 auto 48px;max-width:720px}}
input{{flex:1;min-width:0;background:#20232d;border:1px solid #353a49;border-radius:14px;padding:17px 20px;color:#fff;font-size:16px;outline:none}}
input:focus{{border-color:#8293ff}}button{{background:#6578ed;color:white;border:0;border-radius:12px;padding:0 22px;font-size:15px;cursor:pointer}}
section{{margin:32px auto;max-width:820px}}h2{{font-size:14px;color:#aeb5c8;font-weight:600;margin:0 0 14px}}
.tiles{{display:grid;grid-template-columns:repeat(auto-fill,minmax(150px,1fr));gap:10px}}
.tile,article{{display:block;padding:16px;background:#1b1e26;border:1px solid #292d38;border-radius:12px;color:#e6e9ff;text-decoration:none;overflow-wrap:anywhere}}
.tile:hover,article:hover{{border-color:#6578ed}}article{{margin:9px 0}}small{{display:block;color:#858b9b;font-size:12px;margin-top:6px;overflow-wrap:anywhere}}
.muted{{color:#858b9b;font-size:13px}}
</style></head><body><div class="brand">VERTEX BROWSER</div><h1>Нова вкладка</h1>
<form action="https://www.google.com/search" method="get"><input name="q" type="search" placeholder="Шукати в інтернеті або ввести адресу" autofocus><button type="submit">Знайти</button></form>
${sections}</body></html>"#);
        html_data_url(&html)
    }

    fn open_history_page(&self) {
        let entries = match self.storage.recent_history(500) {
            Ok(entries) => entries,
            Err(error) => {
                log::error!("could not read browsing history: {error}");
                return;
            }
        };
        let mut items = String::new();
        for entry in entries {
            let title = html_escape(if entry.title.trim().is_empty() { &entry.url } else { &entry.title });
            let url_text = html_escape(&entry.url);
            let href = html_escape(&entry.url);
            items.push_str(&format!(
                "<article><a href=\"{href}\">{title}</a><small>{url_text} · {count} visits</small></article>",
                count = entry.visit_count
            ));
        }
        if items.is_empty() { items.push_str("<p class=\"empty\">Історія поки порожня</p>"); }
        let html = internal_page("Історія", &items);
        self.open_internal_page(html);
    }

    fn open_bookmarks_page(&self) {
        let entries = match self.storage.all_bookmarks() {
            Ok(entries) => entries,
            Err(error) => {
                log::error!("could not read bookmarks: {error}");
                return;
            }
        };
        let mut items = String::new();
        for entry in entries {
            let title = html_escape(if entry.title.trim().is_empty() { &entry.url } else { &entry.title });
            let url_text = html_escape(&entry.url);
            let href = html_escape(&entry.url);
            items.push_str(&format!("<article><a href=\"{href}\">{title}</a><small>{url_text}</small></article>"));
        }
        if items.is_empty() { items.push_str("<p class=\"empty\">Закладок поки немає. Натисни Ctrl+D на сторінці, щоб додати її.</p>"); }
        self.open_internal_page(internal_page("Закладки", &items));
    }

    fn open_downloads_page(&self) {
        let entries = match self.storage.recent_downloads(500) {
            Ok(entries) => entries,
            Err(error) => {
                log::error!("could not read downloads: {error}");
                return;
            }
        };
        let mut items = String::new();
        for entry in entries {
            let name = html_escape(&entry.file_name);
            let url = html_escape(&entry.url);
            let path = html_escape(&entry.file_path);
            let (status, progress) = match entry.status.as_str() {
                "queued" => ("У черзі", "Очікує початку".to_owned()),
                "downloading" => ("Завантажується", format_download_progress(entry.received_bytes, entry.total_bytes)),
                "completed" => ("Завершено", format_download_progress(entry.received_bytes, entry.total_bytes)),
                "cancelled" => ("Скасовано", "Завантаження скасовано".to_owned()),
                _ => ("Помилка", "Не вдалося завершити завантаження".to_owned()),
            };
            items.push_str(&format!(
                "<article><strong>{name}</strong><small>{status} · {progress}</small><small>{path}</small><small>{url}</small></article>"
            ));
        }
        if items.is_empty() {
            items.push_str("<p class=\"empty\">Завантажень поки немає.</p>");
        }
        self.open_internal_page(internal_page("Завантаження", &items));
    }

    fn adblock_enabled(&self) -> bool {
        !matches!(self.storage.get_setting("adblock_enabled"), Ok(Some(value)) if value == "false")
    }

    fn toggle_adblock(&self) {
        let enabled = !self.adblock_enabled();
        if let Err(error) = self.storage.set_setting("adblock_enabled", if enabled { "true" } else { "false" }) {
            log::error!("could not save ad blocker setting: {error}");
            return;
        }
        if let Some(view) = self.active_webview() {
            view.reload();
            self.servo.spin_event_loop();
        }
        self.window.request_redraw();
    }

    fn site_adblock_enabled(&self, webview: &WebView) -> bool {
        if !self.adblock_enabled() {
            return false;
        }
        let Some(host) = webview.url().and_then(|url| url.host_str().map(str::to_owned)) else {
            return true;
        };
        let key = format!("adblock_disabled:{host}");
        !matches!(self.storage.get_setting(&key), Ok(Some(value)) if value == "true")
    }

    fn toggle_site_adblock(&self) {
        let Some(host) = self.active_webview().and_then(|view| view.url())
            .and_then(|url| url.host_str().map(str::to_owned)) else {
            return;
        };
        let key = format!("adblock_disabled:{host}");
        let disabled = matches!(self.storage.get_setting(&key), Ok(Some(value)) if value == "true");
        if let Err(error) = self.storage.set_setting(&key, if disabled { "false" } else { "true" }) {
            log::error!("could not save site ad blocker setting: {error}");
            return;
        }
        if let Some(view) = self.active_webview() {
            view.reload();
            self.servo.spin_event_loop();
        }
        self.window.request_redraw();
    }

    fn begin_address_edit(&self) {
        let current = self.active_webview().and_then(|view| view.url())
            .filter(|url| matches!(url.scheme(), "http" | "https"))
            .map(|url| url.to_string()).unwrap_or_default();
        *self.address_bar.borrow_mut() = current;
        self.egui.borrow().egui_ctx.memory_mut(|memory| {
            memory.request_focus(egui::Id::new("vertex-address-bar"));
        });
        self.window.request_redraw();
    }

    fn draw_chrome(self: &Rc<Self>) {
        let active = self.active_webview();
        let current_bookmarked = active.as_ref()
            .and_then(|view| view.url())
            .filter(|url| matches!(url.scheme(), "http" | "https"))
            .and_then(|url| self.storage.is_bookmarked(url.as_str()).ok())
            .unwrap_or(false);
        let tab_labels: Vec<(usize, String, bool, bool)> = self.webviews.borrow().iter().enumerate()
            .map(|(index, view)| {
                let title = view.page_title()
                    .filter(|title| !title.trim().is_empty())
                    .or_else(|| view.url().and_then(|url| url.host_str().map(str::to_owned)))
                    .unwrap_or_else(|| "Нова вкладка".to_owned());
                let loading = matches!(view.load_status(), LoadStatus::Started | LoadStatus::HeadParsed);
                (index, title, index == self.active_tab.get(), loading)
            })
            .collect();

        let initial_active_tab = self.active_tab.get();
        let mut address = self.address_bar.borrow().clone();

        let mut chrome = self.egui.borrow_mut();
        chrome.run(&self.window, |ui| {
            ui.style_mut().visuals.dark_mode = true;
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);

            // The page is composited from an offscreen viewport below this chrome.
            // Keep the chrome backing opaque so no stale window pixels show through.
            let toolbar_cover = egui::Rect::from_min_size(
                ui.min_rect().min,
                egui::vec2(ui.available_width(), 82.0),
            );
            ui.painter().rect_filled(
                toolbar_cover,
                0.0,
                egui::Color32::from_rgb(24, 26, 33),
            );
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    egui::ScrollArea::horizontal()
                        .id_salt("vertex-tab-strip")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for (index, title, selected, loading) in &tab_labels {
                                    let short_title: String = title.chars().take(18).collect();
                                    ui.horizontal(|ui| {
                                        let tab_title = if *loading { format!("◌ {short_title}") } else { short_title };
                                        let label = if *selected {
                                            egui::RichText::new(format!("● {tab_title}")).strong()
                                        } else {
                                            egui::RichText::new(tab_title)
                                        };
                                        if ui.add(egui::Button::new(label).fill(if *selected {
                                            egui::Color32::from_rgb(54, 62, 91)
                                        } else {
                                            egui::Color32::from_rgb(35, 38, 48)
                                        })).clicked() {
                                            self.activate_tab(*index);
                                        }
                                        if ui.small_button("×").on_hover_text("Закрити вкладку").clicked() {
                                            self.close_tab_at(*index);
                                        }
                                        ui.menu_button("⋯", |ui| {
                                            if ui.button("Закрити вкладку").clicked() {
                                                self.close_tab_at(*index);
                                                ui.close();
                                            }
                                            if ui.button("Відновити закриту вкладку").clicked() {
                                                self.reopen_closed_tab();
                                                ui.close();
                                            }
                                        });
                                    });
                                }
                            });
                        });
                    if ui.button("+").clicked() {
                        self.create_tab(self.new_tab_url());
                    }
                    if ui.add_enabled(!self.closed_tabs.borrow().is_empty(), egui::Button::new("↶"))
                        .on_hover_text("Відновити закриту вкладку (Ctrl+Shift+T)").clicked()
                    {
                        self.reopen_closed_tab();
                    }
                });
                ui.horizontal(|ui| {
                    if ui.add_enabled(active.as_ref().is_some_and(|view| view.can_go_back()), egui::Button::new("←")).clicked() {
                        if let Some(view) = &active { view.go_back(1); }
                    }
                    if ui.add_enabled(active.as_ref().is_some_and(|view| view.can_go_forward()), egui::Button::new("→")).clicked() {
                        if let Some(view) = &active { view.go_forward(1); }
                    }
                    if ui.button("⟳").clicked() {
                        if let Some(view) = &active { view.reload(); }
                    }
                    if ui.button("⌂").clicked() {
                        address.clear();
                        if let Some(view) = &active { view.load(self.new_tab_url()); }
                    }
                    if ui.add(egui::Button::new(if current_bookmarked { "★" } else { "☆" })
                        .fill(if current_bookmarked {
                            egui::Color32::from_rgb(80, 72, 38)
                        } else {
                            egui::Color32::from_rgb(35, 38, 48)
                        })).clicked() {
                        if let Some(view) = &active {
                            if let Some(url) = view.url().filter(|url| matches!(url.scheme(), "http" | "https")) {
                                let result = self.storage.is_bookmarked(url.as_str()).and_then(|saved| {
                                    if saved {
                                        self.storage.remove_bookmark(url.as_str())
                                    } else {
                                        let title = view.page_title().unwrap_or_else(|| {
                                            url.host_str().unwrap_or("Saved page").to_owned()
                                        });
                                        self.storage.add_bookmark(url.as_str(), &title)
                                    }
                                });
                                if let Err(error) = result {
                                    log::error!("could not toggle bookmark from toolbar: {error}");
                                }
                            }
                        }
                    }
                    let zoom = active.as_ref().map(|view| view.page_zoom()).unwrap_or(1.0);
                    if ui.button("−").on_hover_text("Зменшити масштаб (Ctrl+-)").clicked() {
                        if let Some(view) = &active { view.set_page_zoom((view.page_zoom() - 0.1).max(0.1)); }
                    }
                    if ui.button(format!("{}%", (zoom * 100.0).round() as u32))
                        .on_hover_text("Скинути масштаб (Ctrl+0)").clicked()
                    {
                        if let Some(view) = &active { view.set_page_zoom(1.0); }
                    }
                    if ui.button("+").on_hover_text("Збільшити масштаб (Ctrl++)").clicked() {
                        if let Some(view) = &active { view.set_page_zoom((view.page_zoom() + 0.1).min(5.0)); }
                    }
                    let adblock_label = if self.adblock_enabled() { "AdBlock: ON" } else { "AdBlock: OFF" };
                    if ui.button(adblock_label).on_hover_text("Глобально увімкнути або вимкнути блокування реклами").clicked() {
                        self.toggle_adblock();
                    }
                    let site_blocking = active.as_ref().is_some_and(|view| self.site_adblock_enabled(view));
                    let site_label = if site_blocking { "На сайті: ON" } else { "На сайті: OFF" };
                    if ui.add_enabled(active.as_ref().and_then(|view| view.url()).is_some(), egui::Button::new(site_label))
                        .on_hover_text("Перемкнути блокування реклами лише для поточного сайту").clicked() {
                        self.toggle_site_adblock();
                    }
                    if ui.button("Фільтри").on_hover_text("Керування власними правилами AdBlock").clicked() {
                        *self.adblock_rules_draft.borrow_mut() = self.adblock_custom_rules.borrow().clone();
                        self.adblock_rules_open.set(true);
                    }
                    if ui.button("Паролі").on_hover_text("Зашифроване локальне сховище паролів").clicked() {
                        if let Some(url) = active.as_ref().and_then(|view| view.url()) {
                            if matches!(url.scheme(), "http" | "https") {
                                *self.vault_origin.borrow_mut() = url.origin().ascii_serialization();
                            }
                        }
                        self.password_vault_open.set(true);
                    }
                    if ui.button("Історія").clicked() {
                        address.clear();
                        self.open_history_page();
                    }
                    if ui.button("Завантаження").clicked() {
                        address.clear();
                        self.open_downloads_page();
                    }
                    if ui.button("Закладки").clicked() {
                        address.clear();
                        self.open_bookmarks_page();
                    }
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut address)
                            .id(egui::Id::new("vertex-address-bar"))
                            .desired_width(ui.available_width() - 8.0)
                            .hint_text("Введи адресу або пошуковий запит"),
                    );
                    if response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                        if !address.trim().is_empty() {
                            if let Some(view) = &active {
                                view.load(parse_navigation_input(&address));
                                self.servo.spin_event_loop();
                            }
                    }
                    }
                });
            });
            if self.adblock_rules_open.get() {
                let mut open = true;
                let mut save = false;
                egui::Window::new("Правила AdBlock")
                    .open(&mut open)
                    .resizable(true)
                    .default_width(520.0)
                    .show(ui.ctx(), |ui| {
                        ui.label("По одному правилу на рядок. Підтримуються домени, шляхи та винятки @@||домен^. Не підтримуються косметичні правила й модифікатори.");
                        ui.add(egui::TextEdit::multiline(&mut *self.adblock_rules_draft.borrow_mut())
                            .desired_rows(14)
                            .desired_width(f32::INFINITY));
                        ui.horizontal(|ui| {
                            if ui.button("Зберегти").clicked() { save = true; }
                            if ui.button("Скасувати").clicked() { self.adblock_rules_open.set(false); }
                        });
                    });
                if !open { self.adblock_rules_open.set(false); }
                if save {
                    let rules = self.adblock_rules_draft.borrow().clone();
                    match self.storage.set_setting("adblock_custom_rules", &rules) {
                        Ok(()) => {
                            *self.adblock_custom_rules.borrow_mut() = rules.clone();
                            *self.adblock_rules.borrow_mut() = adblock::RuleSet::new(&rules);
                            self.adblock_rules_open.set(false);
                            if let Some(view) = self.active_webview() { view.reload(); }
                            self.servo.spin_event_loop();
                        }
                        Err(error) => log::error!("could not save custom AdBlock rules: {error}"),
                    }
                }
            }
            self.draw_password_vault_window(ui.ctx());
        });
        chrome.paint(&self.window);
        if self.active_tab.get() == initial_active_tab {
            *self.address_bar.borrow_mut() = address;
        }
    }

    fn draw_password_vault_window(&self, ctx: &egui::Context) {
        if !self.password_vault_open.get() {
            return;
        }

        let status = self.password_vault.status(&self.storage);
        let unlocked = status.as_ref().is_ok_and(|value| value.is_unlocked);
        let search = self.vault_search.borrow().clone();
        let entries = if unlocked {
            self.password_vault.list_credentials(&self.storage, Some(&search))
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let mut open = true;
        let mut setup = false;
        let mut unlock = false;
        let mut lock = false;
        let mut save = false;
        let mut show_passwords = self.vault_show_passwords.get();
        let mut delete_id: Option<String> = None;

        egui::Window::new("Менеджер паролів")
            .open(&mut open)
            .resizable(true)
            .default_width(560.0)
            .default_height(520.0)
            .show(ctx, |ui| {
                match &status {
                    Ok(value) if !value.is_configured => {
                        ui.label("Створи майстер-пароль. Він не зберігається у відкритому вигляді — з нього виводиться ключ шифрування.");
                        ui.add(egui::TextEdit::singleline(&mut *self.vault_master_password.borrow_mut())
                            .password(true).hint_text("Майстер-пароль"));
                        ui.add(egui::TextEdit::singleline(&mut *self.vault_confirm_password.borrow_mut())
                            .password(true).hint_text("Повтори майстер-пароль"));
                        if ui.button("Створити зашифроване сховище").clicked() {
                            setup = true;
                        }
                    }
                    Ok(value) if !value.is_unlocked => {
                        ui.label("Сховище заблоковане.");
                        ui.add(egui::TextEdit::singleline(&mut *self.vault_master_password.borrow_mut())
                            .password(true).hint_text("Майстер-пароль"));
                        if ui.button("Розблокувати").clicked() {
                            unlock = true;
                        }
                    }
                    Ok(value) => {
                        ui.horizontal(|ui| {
                            ui.label(format!("Записів у сховищі: {}", value.entry_count));
                            if ui.button("Заблокувати").clicked() {
                                lock = true;
                            }
                        });
                        ui.separator();
                        egui::CollapsingHeader::new("Додати пароль")
                            .default_open(true)
                            .show(ui, |ui| {
                                ui.label("Адреса сайту");
                                ui.text_edit_singleline(&mut *self.vault_origin.borrow_mut());
                                ui.label("Назва запису");
                                ui.text_edit_singleline(&mut *self.vault_title.borrow_mut());
                                ui.label("Логін");
                                ui.text_edit_singleline(&mut *self.vault_username.borrow_mut());
                                ui.label("Пароль");
                                ui.add(egui::TextEdit::singleline(&mut *self.vault_password.borrow_mut())
                                    .password(true));
                                ui.label("Нотатки (необов'язково)");
                                ui.text_edit_singleline(&mut *self.vault_notes.borrow_mut());
                                if ui.button("Зберегти пароль").clicked() {
                                    save = true;
                                }
                            });
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("Пошук:");
                            ui.text_edit_singleline(&mut *self.vault_search.borrow_mut());
                            ui.checkbox(&mut show_passwords, "Показувати паролі");
                        });
                        egui::ScrollArea::vertical().max_height(230.0).show(ui, |ui| {
                            for entry in &entries {
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.strong(&entry.title);
                                        ui.label(&entry.origin);
                                        if ui.small_button("Видалити").clicked() {
                                            delete_id = Some(entry.id.clone());
                                        }
                                    });
                                    ui.label(format!("Логін: {}", entry.username));
                                    if show_passwords {
                                        ui.label(format!("Пароль: {}", entry.password));
                                    } else {
                                        ui.label("Пароль: ••••••••");
                                    }
                                    if let Some(notes) = &entry.notes {
                                        ui.label(format!("Нотатки: {notes}"));
                                    }
                                });
                            }
                            if entries.is_empty() {
                                ui.label("Записів не знайдено.");
                            }
                        });
                    }
                    Err(error) => {
                        ui.colored_label(egui::Color32::LIGHT_RED, error);
                    }
                }
                let message = self.vault_message.borrow();
                if !message.is_empty() {
                    ui.separator();
                    ui.label(message.as_str());
                }
            });

        self.vault_show_passwords.set(show_passwords);
        if !open {
            self.password_vault_open.set(false);
        }
        if setup {
            let master = Zeroizing::new(self.vault_master_password.borrow().clone());
            let confirm = Zeroizing::new(self.vault_confirm_password.borrow().clone());
            let result = if master != confirm {
                Err("Майстер-паролі не збігаються".to_owned())
            } else {
                self.password_vault.setup_master_password(&self.storage, master.as_str())
            };
            *self.vault_message.borrow_mut() = result.map(|_| {
                self.vault_master_password.borrow_mut().zeroize();
                self.vault_confirm_password.borrow_mut().zeroize();
                "Сховище створено й розблоковано.".to_owned()
            }).unwrap_or_else(|error| error);
        }
        if unlock {
            let master = Zeroizing::new(self.vault_master_password.borrow().clone());
            let result = self.password_vault.unlock(&self.storage, master.as_str());
            self.vault_master_password.borrow_mut().zeroize();
            *self.vault_message.borrow_mut() = result.map(|_| "Сховище розблоковано.".to_owned())
                .unwrap_or_else(|error| error);
        }
        if lock {
            self.password_vault.lock();
            self.vault_show_passwords.set(false);
            self.vault_master_password.borrow_mut().zeroize();
            self.vault_confirm_password.borrow_mut().zeroize();
            self.vault_password.borrow_mut().zeroize();
            *self.vault_message.borrow_mut() = "Сховище заблоковано.".to_owned();
        }
        if save {
            let origin = self.vault_origin.borrow().clone();
            let title = self.vault_title.borrow().clone();
            let username = Zeroizing::new(self.vault_username.borrow().clone());
            let password = Zeroizing::new(self.vault_password.borrow().clone());
            let notes = Zeroizing::new(self.vault_notes.borrow().clone());
            let result = self.password_vault.save_credential(
                &self.storage, &origin, &title, username.as_str(), password.as_str(), Some(notes.as_str()),
            );
            match result {
                Ok(_) => {
                    self.vault_password.borrow_mut().zeroize();
                    self.vault_notes.borrow_mut().clear();
                    *self.vault_message.borrow_mut() = "Пароль зашифровано та збережено.".to_owned();
                }
                Err(error) => *self.vault_message.borrow_mut() = error,
            }
        }
        if let Some(id) = delete_id {
            let result = self.password_vault.delete_credential(&self.storage, &id);
            *self.vault_message.borrow_mut() = result.map(|_| "Запис видалено.".to_owned())
                .unwrap_or_else(|error| error);
        }
    }

    fn toolbar_height_physical(&self) -> u32 {
        (82.0 * self.window.scale_factor()).round() as u32
    }

    fn page_size(&self, window_size: winit::dpi::PhysicalSize<u32>) -> winit::dpi::PhysicalSize<u32> {
        winit::dpi::PhysicalSize::new(
            window_size.width.max(1),
            window_size.height.saturating_sub(self.toolbar_height_physical()).max(1),
        )
    }

    fn cursor_over_page(&self) -> bool {
        self.cursor_position.get().y >= self.toolbar_height_physical() as f64
    }

    fn cursor_point(&self) -> WebViewPoint {
        let position = self.cursor_position.get();
        let page_y = position.y - self.toolbar_height_physical() as f64;
        WebViewPoint::Device(DevicePoint::new(position.x as f32, page_y.max(0.0) as f32))
    }

    fn handle_keyboard(&self, event: winit::event::KeyEvent, forward_to_page: bool) {
        let Some(webview) = self.active_webview() else {
            return;
        };

        let modifiers = self.modifiers.get();
        if event.state == ElementState::Pressed {
            let is_control = modifiers.contains(Modifiers::CONTROL);
            let is_alt = modifiers.contains(Modifiers::ALT);

            // Native browser shortcuts are intercepted before page delivery.
            if is_control && matches!(&event.logical_key, WinitKey::Character(s) if s.eq_ignore_ascii_case("l")) {
                self.begin_address_edit();
                return;
            }
            if is_control && matches!(&event.logical_key, WinitKey::Character(s) if s.eq_ignore_ascii_case("r")) {
                webview.reload();
                return;
            }
            if is_control && matches!(&event.logical_key, WinitKey::Character(s) if s == "0") {
                webview.set_page_zoom(1.0);
                self.window.request_redraw();
                return;
            }
            if is_control && matches!(&event.logical_key, WinitKey::Character(s) if s == "-" || s == "_") {
                webview.set_page_zoom((webview.page_zoom() - 0.1).max(0.1));
                self.window.request_redraw();
                return;
            }
            if is_control && matches!(&event.logical_key, WinitKey::Character(s) if s == "+" || s == "=") {
                webview.set_page_zoom((webview.page_zoom() + 0.1).min(5.0));
                self.window.request_redraw();
                return;
            }
            if is_control && matches!(&event.logical_key, WinitKey::Character(s) if s.eq_ignore_ascii_case("t")) {
                // Tab creation is handled by the event loop where the shared state is available.
                return;
            }
            if is_control && matches!(&event.logical_key, WinitKey::Character(s) if s.eq_ignore_ascii_case("w")) {
                return;
            }
            if is_control && matches!(&event.logical_key, WinitKey::Character(s) if s.eq_ignore_ascii_case("h")) {
                self.open_history_page();
                return;
            }
            if is_control && matches!(&event.logical_key, WinitKey::Character(s) if s.eq_ignore_ascii_case("j")) {
                self.open_downloads_page();
                return;
            }
            if is_control && modifiers.contains(Modifiers::SHIFT)
                && matches!(&event.logical_key, WinitKey::Character(s) if s.eq_ignore_ascii_case("b"))
            {
                self.open_bookmarks_page();
                return;
            }
            if is_control && matches!(&event.logical_key, WinitKey::Character(s) if s.eq_ignore_ascii_case("d")) {
                if let Some(url) = webview.url().filter(|url| matches!(url.scheme(), "http" | "https")) {
                    let result = self.storage.is_bookmarked(url.as_str()).and_then(|saved| {
                        if saved {
                            self.storage.remove_bookmark(url.as_str())
                        } else {
                            let title = webview.page_title().unwrap_or_else(|| url.host_str().unwrap_or("Saved page").to_owned());
                            self.storage.add_bookmark(url.as_str(), &title)
                        }
                    });
                    if let Err(error) = result {
                        log::error!("could not toggle bookmark: {error}");
                    }
                }
                return;
            }
            if is_alt && matches!(&event.logical_key, WinitKey::Named(WinitNamedKey::ArrowLeft)) {
                webview.go_back(1);
                return;
            }
            if is_alt && matches!(&event.logical_key, WinitKey::Named(WinitNamedKey::ArrowRight)) {
                webview.go_forward(1);
                return;
            }
            if matches!(&event.logical_key, WinitKey::Named(WinitNamedKey::F5)) {
                webview.reload();
                return;
            }
        }

        let key = match &event.logical_key {
            WinitKey::Character(value) => Key::Character(value.to_string()),
            WinitKey::Named(WinitNamedKey::Space) => Key::Character(" ".to_owned()),
            WinitKey::Named(named) => {
                let named = match named {
                    WinitNamedKey::Backspace => NamedKey::Backspace,
                    WinitNamedKey::Tab => NamedKey::Tab,
                    WinitNamedKey::Enter => NamedKey::Enter,
                    WinitNamedKey::Escape => NamedKey::Escape,
                    WinitNamedKey::ArrowDown => NamedKey::ArrowDown,
                    WinitNamedKey::ArrowLeft => NamedKey::ArrowLeft,
                    WinitNamedKey::ArrowRight => NamedKey::ArrowRight,
                    WinitNamedKey::ArrowUp => NamedKey::ArrowUp,
                    WinitNamedKey::Delete => NamedKey::Delete,
                    WinitNamedKey::Home => NamedKey::Home,
                    WinitNamedKey::End => NamedKey::End,
                    WinitNamedKey::PageDown => NamedKey::PageDown,
                    WinitNamedKey::PageUp => NamedKey::PageUp,
                    WinitNamedKey::Insert => NamedKey::Insert,
                    WinitNamedKey::F1 => NamedKey::F1,
                    WinitNamedKey::F2 => NamedKey::F2,
                    WinitNamedKey::F3 => NamedKey::F3,
                    WinitNamedKey::F4 => NamedKey::F4,
                    WinitNamedKey::F5 => NamedKey::F5,
                    WinitNamedKey::F6 => NamedKey::F6,
                    WinitNamedKey::F7 => NamedKey::F7,
                    WinitNamedKey::F8 => NamedKey::F8,
                    WinitNamedKey::F9 => NamedKey::F9,
                    WinitNamedKey::F10 => NamedKey::F10,
                    WinitNamedKey::F11 => NamedKey::F11,
                    WinitNamedKey::F12 => NamedKey::F12,
                    WinitNamedKey::CapsLock => NamedKey::CapsLock,
                    WinitNamedKey::Shift => NamedKey::Shift,
                    WinitNamedKey::Control => NamedKey::Control,
                    WinitNamedKey::Alt => NamedKey::Alt,
                    WinitNamedKey::Meta => NamedKey::Meta,
                    _ => NamedKey::Unidentified,
                };
                Key::Named(named)
            }
            _ => Key::Named(NamedKey::Unidentified),
        };

        let state = match event.state {
            ElementState::Pressed => KeyState::Down,
            ElementState::Released => KeyState::Up,
        };

        let keyboard_event = KeyboardEvent::new_without_event(
            state,
            key,
            match event.physical_key {
                PhysicalKey::Code(code) => code_from_winit(code),
                PhysicalKey::Unidentified(_) => Code::Unidentified,
            },
            Location::Standard,
            modifiers,
            event.repeat,
            false,
        );
        if forward_to_page {
            webview.notify_input_event(InputEvent::Keyboard(keyboard_event));
        }
    }
}

impl servo::WebViewDelegate for BrowserState {
    fn load_web_resource(&self, webview: WebView, load: servo::WebResourceLoad) {
        if !self.site_adblock_enabled(&webview) {
            return;
        }
        let Ok(url) = Url::parse(&load.request().url.to_string()) else {
            return;
        };
        if self.adblock_rules.borrow().is_blocked(&url) {
            log::debug!("AdBlock blocked resource: {url}");
            // An empty successful response prevents the blocked resource from reaching the page.
            // Dropping the interception sends FinishLoad to Servo.
            let _ = load.intercept(servo::WebResourceResponse::new(url));
        }
    }

    fn notify_new_frame_ready(&self, _webview: WebView) {
        self.window.request_redraw();
    }

    fn notify_load_status_changed(&self, _webview: WebView, _status: LoadStatus) {
        self.window.request_redraw();
    }

    fn notify_url_changed(&self, webview: WebView, url: Url) {
        let is_active = self.active_webview()
            .and_then(|active| active.url())
            .as_ref() == Some(&url);
        if is_active {
            self.update_window_title(&webview);
            *self.address_bar.borrow_mut() = if matches!(url.scheme(), "http" | "https") {
                url.to_string()
            } else {
                String::new()
            };
        }
        let title = webview.page_title().unwrap_or_else(|| url.host_str().unwrap_or("New page").to_owned());
        if let Err(error) = self.storage.record_visit(url.as_str(), &title) {
            log::error!("could not record browsing history: {error}");
        }
    }

    fn notify_page_title_changed(&self, webview: WebView, title: Option<String>) {
        let url = webview.url();
        let is_active = self.active_webview()
            .and_then(|active| active.url()) == url;
        if is_active {
            self.update_window_title(&webview);
        }
        if let (Some(url), Some(title)) = (url, title) {
            if let Err(error) = self.storage.update_page_title(url.as_str(), &title) {
                log::error!("could not update history title: {error}");
            }
        }
    }
}

enum VertexApp {
    Waiting(ServoWaker),
    Running(Rc<BrowserState>),
}

impl VertexApp {
    fn new(event_loop: &EventLoop<ServoWakeup>) -> Self {
        Self::Waiting(ServoWaker(event_loop.create_proxy()))
    }

    fn initialize(&mut self, event_loop: &ActiveEventLoop) {
        let Self::Waiting(waker) = self else {
            return;
        };

        let display_handle = event_loop
            .display_handle()
            .expect("failed to get display handle");
        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Vertex Browser — Servo")
                    .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0)),
            )
            .expect("failed to create native window");
        window.set_ime_allowed(true);

        let window_handle = window
            .window_handle()
            .expect("failed to get window handle");
        let rendering_context = Rc::new(
            WindowRenderingContext::new(display_handle, window_handle, window.inner_size())
                .expect("failed to create Servo rendering context"),
        );
        let _ = rendering_context.make_current();

        let servo = ServoBuilder::default()
            .event_loop_waker(Box::new(waker.clone()))
            .build();
        servo.setup_logging();

        let profile_dir = default_profile_dir();
        let storage = BrowserStorage::open(&profile_dir.join("profile.sqlite3"))
            .expect("failed to open Vertex browser profile database");

        let window_size = window.inner_size();
        let toolbar_height = (82.0 * window.scale_factor()).round() as u32;
        let page_context_size = winit::dpi::PhysicalSize::new(
            window_size.width.max(1),
            window_size.height.saturating_sub(toolbar_height).max(1),
        );
        let page_context = Rc::new(rendering_context.offscreen_context(page_context_size));

        let chrome = egui_glow::winit::EguiGlow::new(
            event_loop,
            rendering_context.glow_gl_api(),
            None,
            None,
            true,
        );
        let custom_adblock_rules = storage.get_setting("adblock_custom_rules")
            .ok().flatten().unwrap_or_default();
        let state = Rc::new(BrowserState {
            window,
            servo,
            rendering_context,
            page_context,
            webviews: RefCell::new(Vec::new()),
            cursor_position: Cell::new(PhysicalPosition::new(0.0, 0.0)),
            modifiers: Cell::new(Modifiers::empty()),
            active_tab: Cell::new(0),
            closed_tabs: RefCell::new(Vec::new()),
            storage,
            egui: RefCell::new(chrome),
            address_bar: RefCell::new(String::new()),
            adblock_rules: RefCell::new(adblock::RuleSet::new(&custom_adblock_rules)),
            adblock_custom_rules: RefCell::new(custom_adblock_rules),
            adblock_rules_draft: RefCell::new(String::new()),
            adblock_rules_open: Cell::new(false),
            password_vault: password_vault::PasswordVault::new(),
            password_vault_open: Cell::new(false),
            vault_master_password: RefCell::new(String::new()),
            vault_confirm_password: RefCell::new(String::new()),
            vault_origin: RefCell::new(String::new()),
            vault_title: RefCell::new(String::new()),
            vault_username: RefCell::new(String::new()),
            vault_password: RefCell::new(String::new()),
            vault_notes: RefCell::new(String::new()),
            vault_search: RefCell::new(String::new()),
            vault_show_passwords: Cell::new(false),
            vault_message: RefCell::new(String::new()),
        });

        let url = match std::env::args().nth(1) {
            Some(start_url) => parse_navigation_input(&start_url),
            None => state.new_tab_url(),
        };

        let webview = WebViewBuilder::new(&state.servo, state.page_context.clone())
            .url(url)
            .hidpi_scale_factor(euclid::Scale::new(state.window.scale_factor() as f32))
            .delegate(state.clone())
            .build();
        webview.set_focused(true);
        *state.address_bar.borrow_mut() = webview.url()
            .filter(|url| matches!(url.scheme(), "http" | "https"))
            .map(|url| url.to_string())
            .unwrap_or_default();
        state.webviews.borrow_mut().push(webview);

        *self = Self::Running(state);
        if let Self::Running(state) = self {
            state.window.request_redraw();
        }
    }
}

fn html_data_url(html: &str) -> Url {
    let encoded: String = url::form_urlencoded::byte_serialize(html.as_bytes()).collect();
    Url::parse(&format!("data:text/html;charset=utf-8,{encoded}"))
        .expect("generated internal page URL is valid")
}

fn html_escape(value: &str) -> String {
    value.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn internal_page(title: &str, items: &str) -> String {
    format!(r#"<!doctype html>
<html lang="uk"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title} — Vertex</title><style>
:root{{color-scheme:dark;font-family:Segoe UI,Arial,sans-serif;background:#111318;color:#e8eaf0}}
*{{box-sizing:border-box}}body{{margin:0;padding:48px max(24px,calc((100vw - 900px)/2));}}
h1{{font-size:30px;font-weight:650;margin:0 0 28px}}.brand{{color:#8b9cff;font-size:12px;letter-spacing:2px;text-transform:uppercase;margin-bottom:12px}}
article{{padding:16px 18px;margin:10px 0;background:#1b1e26;border:1px solid #292d38;border-radius:12px}}
a{{color:#e6e9ff;text-decoration:none;font-size:15px}}a:hover{{color:#9eacff}}
small{{display:block;color:#858b9b;font-size:12px;margin-top:7px;overflow-wrap:anywhere}}
.empty{{color:#858b9b;padding:24px 0}}
</style></head><body><div class="brand">VERTEX BROWSER</div><h1>{title}</h1>{items}</body></html>"#)
}

fn format_download_progress(received: u64, total: Option<u64>) -> String {
    match total.filter(|total| *total > 0) {
        Some(total) => format!(
            "{}% · {} / {} bytes",
            ((received as f64 / total as f64) * 100.0).clamp(0.0, 100.0).round() as u32,
            received,
            total
        ),
        None => format!("{received} bytes"),
    }
}

fn parse_navigation_input(input: &str) -> Url {
    let trimmed = input.trim();
    if let Ok(url) = Url::parse(trimmed) {
        if url.scheme() == "http" || url.scheme() == "https" {
            return url;
        }
    }

    // Accept ordinary hostnames, localhost and IP literals without requiring a scheme.
    // A bare single word remains a search query, matching common browser behaviour.
    let looks_like_host = !trimmed.is_empty()
        && !trimmed.contains(char::is_whitespace)
        && (trimmed.contains('.')
            || trimmed.eq_ignore_ascii_case("localhost")
            || trimmed.to_ascii_lowercase().starts_with("localhost:")
            || trimmed.starts_with('['));

    if looks_like_host {
        if let Ok(url) = Url::parse(&format!("https://{trimmed}")) {
            if url.host_str().is_some() {
                return url;
            }
        }
    }

    let encoded: String = url::form_urlencoded::byte_serialize(trimmed.as_bytes()).collect();
    Url::parse(&format!("https://www.google.com/search?q={encoded}"))
        .expect("constructed search URL is valid")
}


fn code_from_winit(code: KeyCode) -> Code {
    match code {
        KeyCode::KeyA => Code::KeyA, KeyCode::KeyB => Code::KeyB,
        KeyCode::KeyC => Code::KeyC, KeyCode::KeyD => Code::KeyD,
        KeyCode::KeyE => Code::KeyE, KeyCode::KeyF => Code::KeyF,
        KeyCode::KeyG => Code::KeyG, KeyCode::KeyH => Code::KeyH,
        KeyCode::KeyI => Code::KeyI, KeyCode::KeyJ => Code::KeyJ,
        KeyCode::KeyK => Code::KeyK, KeyCode::KeyL => Code::KeyL,
        KeyCode::KeyM => Code::KeyM, KeyCode::KeyN => Code::KeyN,
        KeyCode::KeyO => Code::KeyO, KeyCode::KeyP => Code::KeyP,
        KeyCode::KeyQ => Code::KeyQ, KeyCode::KeyR => Code::KeyR,
        KeyCode::KeyS => Code::KeyS, KeyCode::KeyT => Code::KeyT,
        KeyCode::KeyU => Code::KeyU, KeyCode::KeyV => Code::KeyV,
        KeyCode::KeyW => Code::KeyW, KeyCode::KeyX => Code::KeyX,
        KeyCode::KeyY => Code::KeyY, KeyCode::KeyZ => Code::KeyZ,
        KeyCode::Digit0 => Code::Digit0, KeyCode::Digit1 => Code::Digit1,
        KeyCode::Digit2 => Code::Digit2, KeyCode::Digit3 => Code::Digit3,
        KeyCode::Digit4 => Code::Digit4, KeyCode::Digit5 => Code::Digit5,
        KeyCode::Digit6 => Code::Digit6, KeyCode::Digit7 => Code::Digit7,
        KeyCode::Digit8 => Code::Digit8, KeyCode::Digit9 => Code::Digit9,
        KeyCode::Enter => Code::Enter, KeyCode::Escape => Code::Escape,
        KeyCode::Backspace => Code::Backspace, KeyCode::Tab => Code::Tab,
        KeyCode::Space => Code::Space, KeyCode::ArrowDown => Code::ArrowDown,
        KeyCode::ArrowLeft => Code::ArrowLeft, KeyCode::ArrowRight => Code::ArrowRight,
        KeyCode::ArrowUp => Code::ArrowUp, KeyCode::Delete => Code::Delete,
        KeyCode::Home => Code::Home, KeyCode::End => Code::End,
        KeyCode::PageDown => Code::PageDown, KeyCode::PageUp => Code::PageUp,
        KeyCode::Insert => Code::Insert, KeyCode::F1 => Code::F1,
        KeyCode::F2 => Code::F2, KeyCode::F3 => Code::F3,
        KeyCode::F4 => Code::F4, KeyCode::F5 => Code::F5,
        KeyCode::F6 => Code::F6, KeyCode::F7 => Code::F7,
        KeyCode::F8 => Code::F8, KeyCode::F9 => Code::F9,
        KeyCode::F10 => Code::F10, KeyCode::F11 => Code::F11,
        KeyCode::F12 => Code::F12, KeyCode::ShiftLeft => Code::ShiftLeft,
        KeyCode::ShiftRight => Code::ShiftRight,
        KeyCode::ControlLeft => Code::ControlLeft,
        KeyCode::ControlRight => Code::ControlRight,
        KeyCode::AltLeft => Code::AltLeft, KeyCode::AltRight => Code::AltRight,
        KeyCode::SuperLeft => Code::MetaLeft, KeyCode::SuperRight => Code::MetaRight,
        _ => Code::Unidentified,
    }
}

fn modifiers_from_winit(mods: ModifiersState) -> Modifiers {
    let mut result = Modifiers::empty();
    result.set(Modifiers::CONTROL, mods.control_key());
    result.set(Modifiers::SHIFT, mods.shift_key());
    result.set(Modifiers::ALT, mods.alt_key());
    result.set(Modifiers::META, mods.super_key());
    result
}

impl ApplicationHandler<ServoWakeup> for VertexApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.initialize(event_loop);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: ServoWakeup) {
        if let Self::Running(state) = self {
            state.servo.spin_event_loop();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Self::Running(state) = self {
            let _ = state.rendering_context.make_current();
            state.egui.borrow_mut().destroy();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Self::Running(state) = self {
            state.servo.spin_event_loop();
        }

        let egui_consumed = if let Self::Running(state) = self {
            let response = state.egui.borrow_mut().on_window_event(&state.window, &event);
            if response.repaint {
                state.window.request_redraw();
            }
            response.consumed
        } else {
            false
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Self::Running(state) = self {
                    if let Some(webview) = state.active_webview() {
                        webview.paint();
                        let page_height = state.page_context.size().height;
                        let page_width = state.page_context.size().width;
                        if let Some(blit_to_window) = state.page_context.render_to_parent_callback() {
                            let gl = state.rendering_context.glow_gl_api();
                            blit_to_window(
                                &gl,
                                euclid::default::Rect::new(
                                    euclid::default::Point2D::new(0, 0),
                                    euclid::default::Size2D::new(page_width as i32, page_height as i32),
                                ),
                            );
                        }
                        state.draw_chrome();
                        state.rendering_context.present();
                    }
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Self::Running(state) = self {
                    let window_size = state.window.inner_size();
                    state.rendering_context.resize(window_size);
                    // Every tab shares the same offscreen surface, but each WebView
                    // keeps its own device scale factor. Update all tabs after a DPI
                    // transition so switching to a background tab cannot restore a
                    // stale scale factor.
                    for webview in state.webviews.borrow().iter() {
                        webview.set_hidpi_scale_factor(euclid::Scale::new(scale_factor as f32));
                    }
                    if let Some(webview) = state.active_webview() {
                        webview.resize(state.page_size(window_size));
                    }
                }
            }
            WindowEvent::Resized(size) => {
                if let Self::Running(state) = self {
                    state.rendering_context.resize(size);
                    if let Some(webview) = state.active_webview() {
                        webview.resize(state.page_size(size));
                    }
                }
            }
            WindowEvent::Focused(focused) => {
                if let Self::Running(state) = self {
                    if let Some(webview) = state.active_webview() {
                        webview.set_focused(focused);
                    }
                }
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                if egui_consumed { return; }
                if let Self::Running(state) = self {
                    if let Some(webview) = state.active_webview() {
                        webview.notify_input_event(InputEvent::Keyboard(
                            KeyboardEvent::from_state_and_key(
                                KeyState::Down,
                                Key::Named(NamedKey::Process),
                            ),
                        ));
                        webview.notify_input_event(InputEvent::Ime(ImeEvent::Commit(text)));
                        webview.notify_input_event(InputEvent::Keyboard(
                            KeyboardEvent::from_state_and_key(
                                KeyState::Up,
                                Key::Named(NamedKey::Process),
                            ),
                        ));
                    }
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                if let Self::Running(state) = self {
                    state.modifiers.set(modifiers_from_winit(modifiers.state()));
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Self::Running(state) = self {
                    if event.state == ElementState::Pressed {
                        let control = state.modifiers.get().contains(Modifiers::CONTROL);
                        let shift = state.modifiers.get().contains(Modifiers::SHIFT);
                        // Check the more specific shortcut first: Ctrl+Shift+T
                        // must restore a closed tab, not create a new one.
                        if control && shift && matches!(&event.logical_key, WinitKey::Character(s) if s.eq_ignore_ascii_case("t")) {
                            state.reopen_closed_tab();
                            return;
                        }
                        if control && matches!(&event.logical_key, WinitKey::Character(s) if s.eq_ignore_ascii_case("t")) {
                            let new_tab = state.new_tab_url();
                            state.create_tab(new_tab);
                            return;
                        }
                        if control && matches!(&event.logical_key, WinitKey::Character(s) if s.eq_ignore_ascii_case("w")) {
                            state.close_active_tab();
                            return;
                        }
                        if control {
                            if let WinitKey::Character(value) = &event.logical_key {
                                if let Some(digit) = value.chars().next().filter(|ch| ('1'..='9').contains(ch)) {
                                    let count = state.tab_count();
                                    let target = if digit == '9' { count.saturating_sub(1) } else { digit.to_digit(10).unwrap_or(1) as usize - 1 };
                                    if target < count {
                                        state.activate_tab(target);
                                    }
                                    return;
                                }
                            }
                        }
                        if control && matches!(&event.logical_key, WinitKey::Named(WinitNamedKey::Tab)) {
                            let count = state.tab_count();
                            if count > 1 {
                                let current = state.active_tab.get();
                                let next = if shift {
                                    (current + count - 1) % count
                                } else {
                                    (current + 1) % count
                                };
                                state.activate_tab(next);
                            }
                            return;
                        }
                    }
                    state.handle_keyboard(event, !egui_consumed);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Self::Running(state) = self {
                    state.cursor_position.set(position);
                    if egui_consumed || !state.cursor_over_page() {
                        if let Some(webview) = state.active_webview() {
                            webview.notify_input_event(InputEvent::MouseLeftViewport(MouseLeftViewportEvent::default()));
                        }
                        return;
                    }
                    if let Some(webview) = state.active_webview() {
                        webview.notify_input_event(InputEvent::MouseMove(MouseMoveEvent::new(
                            state.cursor_point(),
                        )));
                    }
                }
            }
            WindowEvent::CursorLeft { .. } => {
                if let Self::Running(state) = self {
                    if let Some(webview) = state.active_webview() {
                        webview.notify_input_event(InputEvent::MouseLeftViewport(
                            MouseLeftViewportEvent::default(),
                        ));
                    }
                }
            }
            WindowEvent::MouseInput {
                state: button_state,
                button,
                ..
            } => {
                if egui_consumed { return; }
                if let Self::Running(state) = self {
                    if !state.cursor_over_page() { return; }
                    let servo_button = match button {
                        winit::event::MouseButton::Left => ServoMouseButton::Left,
                        winit::event::MouseButton::Right => ServoMouseButton::Right,
                        winit::event::MouseButton::Middle => ServoMouseButton::Middle,
                        _ => return,
                    };
                    let action = match button_state {
                        ElementState::Pressed => MouseButtonAction::Down,
                        ElementState::Released => MouseButtonAction::Up,
                    };
                    if let Some(webview) = state.active_webview() {
                        webview.notify_input_event(InputEvent::MouseButton(MouseButtonEvent::new(
                            action,
                            servo_button,
                            state.cursor_point(),
                        )));
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if egui_consumed { return; }
                if let Self::Running(state) = self {
                    if !state.cursor_over_page() { return; }
                    let (dx, dy, mode) = match delta {
                        MouseScrollDelta::LineDelta(x, y) => {
                            ((x as f64) * 38.0, (y as f64) * 38.0, WheelMode::DeltaLine)
                        }
                        MouseScrollDelta::PixelDelta(position) => {
                            (position.x, position.y, WheelMode::DeltaPixel)
                        }
                    };
                    if let Some(webview) = state.active_webview() {
                        webview.notify_input_event(InputEvent::Wheel(WheelEvent::new(
                            WheelDelta { x: dx, y: dy, z: 0.0, mode },
                            state.cursor_point(),
                            state.modifiers.get(),
                        )));
                    }
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
struct ServoWaker(EventLoopProxy<ServoWakeup>);

#[derive(Debug)]
struct ServoWakeup;

impl EventLoopWaker for ServoWaker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }

    fn wake(&self) {
        if let Err(error) = self.0.send_event(ServoWakeup) {
            log::warn!("could not wake winit event loop: {error}");
        }
    }
}


#[cfg(test)]
mod tests {
    use super::parse_navigation_input;


    #[test]
    fn escapes_html_in_internal_pages() {
        assert_eq!(
            super::html_escape("<script>&'"),
            "&lt;script&gt;&amp;&#39;"
        );
    }

    #[test]
    fn internal_pages_use_dark_styling_and_supplied_content() {
        let page = super::internal_page("Історія", "<article>Example</article>");
        assert!(page.contains("color-scheme:dark"));
        assert!(page.contains("<article>Example</article>"));
        assert!(page.contains("<h1>Історія</h1>"));
    }

    #[test]
    fn formats_download_progress_with_and_without_total() {
        assert_eq!(format_download_progress(512, Some(1024)), "50% · 512 / 1024 bytes");
        assert_eq!(format_download_progress(2048, None), "2048 bytes");
        assert_eq!(format_download_progress(1200, Some(1000)), "100% · 1200 / 1000 bytes");
    }

    #[test]
    fn accepts_explicit_https_url() {
        let url = parse_navigation_input("https://example.org/path");
        assert_eq!(url.as_str(), "https://example.org/path");
    }

    #[test]
    fn upgrades_plain_hostname_to_https() {
        let url = parse_navigation_input("example.org");
        assert_eq!(url.as_str(), "https://example.org/");
    }

    #[test]
    fn accepts_localhost_with_port() {
        let url = parse_navigation_input("localhost:3000");
        assert_eq!(url.as_str(), "https://localhost:3000/");
    }

    #[test]
    fn accepts_ipv4_with_port() {
        let url = parse_navigation_input("127.0.0.1:8080");
        assert_eq!(url.as_str(), "https://127.0.0.1:8080/");
    }

    #[test]
    fn turns_plain_text_into_search() {
        let url = parse_navigation_input("servo browser engine");
        assert_eq!(url.host_str(), Some("www.google.com"));
        assert_eq!(
            url.query_pairs().find(|(key, _)| key == "q").map(|(_, value)| value.into_owned()),
            Some("servo browser engine".to_owned())
        );
    }

    #[test]
    fn encodes_unicode_search_terms() {
        let url = parse_navigation_input("браузер");
        assert_eq!(
            url.query_pairs().find(|(key, _)| key == "q").map(|(_, value)| value.into_owned()),
            Some("браузер".to_owned())
        );
    }
}
