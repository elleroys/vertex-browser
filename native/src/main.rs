use std::{
    cell::{Cell, RefCell},
    error::Error,
    rc::Rc,
};

use servo::{
    DevicePoint, EventLoopWaker, InputEvent, Modifiers,
    MouseButton as ServoMouseButton, MouseButtonAction, MouseButtonEvent,
    MouseLeftViewportEvent, MouseMoveEvent, Servo, ServoBuilder, WebView,
    WebViewBuilder, WebViewPoint, WheelDelta, WheelEvent, WheelMode,
    WindowRenderingContext,
};
use url::Url;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
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
    cursor_position: Cell<PhysicalPosition<f64>>,
}

impl BrowserState {
    fn active_webview(&self) -> Option<WebView> {
        self.webviews.borrow().last().cloned()
    }

    fn cursor_point(&self) -> WebViewPoint {
        let position = self.cursor_position.get();
        WebViewPoint::Device(DevicePoint::new(position.x as f32, position.y as f32))
    }
}

impl servo::WebViewDelegate for BrowserState {
    fn notify_new_frame_ready(&self, _webview: WebView) {
        self.window.request_redraw();
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
                    .with_title("Vertex Browser — Servo prototype")
                    .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0)),
            )
            .expect("failed to create native window");

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
        });

        let start_url = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "https://example.com".to_owned());
        let url = Url::parse(&start_url).unwrap_or_else(|_| {
            Url::parse("https://example.com").expect("static fallback URL is valid")
        });

        let webview = WebViewBuilder::new(&state.servo, state.rendering_context.clone())
            .url(url)
            .hidpi_scale_factor(euclid::Scale::new(state.window.scale_factor() as f32))
            .delegate(state.clone())
            .build();
        state.webviews.borrow_mut().push(webview);

        *self = Self::Running(state);
        if let Self::Running(state) = self {
            state.window.request_redraw();
        }
    }
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
                            Modifiers::default(),
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
