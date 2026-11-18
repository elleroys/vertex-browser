use std::{
    cell::{Cell, RefCell},
    error::Error,
    rc::Rc,
};

use servo::{
    Code, DevicePoint, EventLoopWaker, InputEvent, Key, KeyState, Location, Modifiers,
    MouseButton as ServoMouseButton, MouseButtonAction, MouseButtonEvent,
    ImeEvent, MouseLeftViewportEvent, MouseMoveEvent, NamedKey, Servo, ServoBuilder,
    WebView, WebViewBuilder, WebViewPoint, WheelDelta, WheelEvent, WheelMode,
    WindowRenderingContext, KeyboardEvent,
};
use url::Url;
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
    webviews: RefCell<Vec<WebView>>,
    active_tab: Cell<usize>,
    cursor_position: Cell<PhysicalPosition<f64>>,
    modifiers: Cell<Modifiers>,
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
        }
        self.active_tab.set(index);
        if let Some(active) = tabs.get(index) {
            active.set_focused(true);
            self.update_window_title(active);
        }
        self.window.request_redraw();
    }

    fn create_tab(self: &Rc<Self>, url: Url) {
        if let Some(active) = self.active_webview() {
            active.set_focused(false);
        }
        let webview = WebViewBuilder::new(&self.servo, self.rendering_context.clone())
            .url(url)
            .hidpi_scale_factor(euclid::Scale::new(self.window.scale_factor() as f32))
            .delegate(self.clone())
            .build();
        let mut tabs = self.webviews.borrow_mut();
        tabs.push(webview);
        let index = tabs.len() - 1;
        self.active_tab.set(index);
        if let Some(active) = tabs.get(index) {
            active.set_focused(true);
            self.update_window_title(active);
        }
        drop(tabs);
        self.window.request_redraw();
    }

    fn close_active_tab(&self) {
        let mut tabs = self.webviews.borrow_mut();
        if tabs.len() <= 1 {
            return;
        }
        let index = self.active_tab.get().min(tabs.len() - 1);
        tabs[index].set_focused(false);
        tabs.remove(index);
        let next = index.min(tabs.len() - 1);
        self.active_tab.set(next);
        if let Some(active) = tabs.get(next) {
            active.set_focused(true);
            self.update_window_title(active);
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

    fn cursor_point(&self) -> WebViewPoint {
        let position = self.cursor_position.get();
        WebViewPoint::Device(DevicePoint::new(position.x as f32, position.y as f32))
    }

    fn handle_keyboard(&self, event: winit::event::KeyEvent) {
        let Some(webview) = self.active_webview() else {
            return;
        };

        let modifiers = self.modifiers.get();
        if event.state == ElementState::Pressed {
            let is_control = modifiers.contains(Modifiers::CONTROL);
            let is_alt = modifiers.contains(Modifiers::ALT);

            // Native browser shortcuts are intercepted before page delivery.
            if is_control && matches!(&event.logical_key, WinitKey::Character(s) if s.eq_ignore_ascii_case("r")) {
                webview.reload();
                return;
            }
            if is_control && matches!(&event.logical_key, WinitKey::Character(s) if s.eq_ignore_ascii_case("t")) {
                // Tab creation is handled by the event loop where the shared state is available.
                return;
            }
            if is_control && matches!(&event.logical_key, WinitKey::Character(s) if s.eq_ignore_ascii_case("w")) {
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
        webview.notify_input_event(InputEvent::Keyboard(keyboard_event));
    }
}

impl servo::WebViewDelegate for BrowserState {
    fn notify_new_frame_ready(&self, _webview: WebView) {
        self.window.request_redraw();
    }

    fn notify_url_changed(&self, webview: WebView, _url: Url) {
        self.update_window_title(&webview);
    }

    fn notify_page_title_changed(&self, webview: WebView, _title: Option<String>) {
        self.update_window_title(&webview);
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

        let state = Rc::new(BrowserState {
            window,
            servo,
            rendering_context,
            webviews: RefCell::new(Vec::new()),
            cursor_position: Cell::new(PhysicalPosition::new(0.0, 0.0)),
            modifiers: Cell::new(Modifiers::empty()),
            active_tab: Cell::new(0),
        });

        let start_url = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "https://example.com".to_owned());
        let url = parse_navigation_input(&start_url);

        let webview = WebViewBuilder::new(&state.servo, state.rendering_context.clone())
            .url(url)
            .hidpi_scale_factor(euclid::Scale::new(state.window.scale_factor() as f32))
            .delegate(state.clone())
            .build();
        webview.set_focused(true);
        state.webviews.borrow_mut().push(webview);

        *self = Self::Running(state);
        if let Self::Running(state) = self {
            state.window.request_redraw();
        }
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

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Self::Running(state) = self {
            state.servo.spin_event_loop();
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Self::Running(state) = self {
                    if let Some(webview) = state.active_webview() {
                        webview.paint();
                        state.rendering_context.present();
                    }
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Self::Running(state) = self {
                    if let Some(webview) = state.active_webview() {
                        webview.set_hidpi_scale_factor(euclid::Scale::new(scale_factor as f32));
                    }
                }
            }
            WindowEvent::Resized(size) => {
                if let Self::Running(state) = self {
                    if let Some(webview) = state.active_webview() {
                        webview.resize(size);
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
                        if control && matches!(&event.logical_key, WinitKey::Character(s) if s.eq_ignore_ascii_case("t")) {
                            state.create_tab(Url::parse("about:blank").expect("valid blank URL"));
                            return;
                        }
                        if control && matches!(&event.logical_key, WinitKey::Character(s) if s.eq_ignore_ascii_case("w")) {
                            state.close_active_tab();
                            return;
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
                    state.handle_keyboard(event);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Self::Running(state) = self {
                    state.cursor_position.set(position);
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
                if let Self::Running(state) = self {
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
                if let Self::Running(state) = self {
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
