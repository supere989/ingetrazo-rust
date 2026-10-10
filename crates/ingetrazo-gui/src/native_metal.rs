//! Opt-in native Metal host using the shared application and egui-winit.
//! Full native world/CAD callbacks and physical macOS acceptance remain open.
use crate::{IngeTrazoApp, session_state::SessionStore};
use ingetrazo_metal::{NativeMetal, egui_painter::EguiPainter, presentation::MetalPresentation};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

mod lifecycle_probe;
/// Engineering probe through actual own-window Cocoa events, not physical input.
pub fn run_document_probe(root: PathBuf, choice: &str) -> Result<(), Box<dyn std::error::Error>> {
    let probe = lifecycle_probe::Probe::new(root, choice)?;
    let initial = Some(probe.document.clone());
    run_inner(initial, Some(probe))
}
pub fn run(initial_file: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    run_inner(initial_file, None)
}
fn run_inner(
    initial_file: Option<PathBuf>,
    probe: Option<lifecycle_probe::Probe>,
) -> Result<(), Box<dyn std::error::Error>> {
    let event = EventLoop::<()>::with_user_event().build()?;
    let ctx = egui::Context::default();
    let proxy = event.create_proxy();
    ctx.set_request_repaint_callback(move |info| {
        if info.delay.is_zero() {
            let _ = proxy.send_event(());
        }
    });
    let mut host = Host {
        state: None,
        initial_file,
        ctx,
        repaint: Instant::now(),
        error: None,
        probe,
    };
    event.run_app(&mut host)?;
    if let Some(error) = host.error {
        return Err(error.into());
    }
    Ok(())
}
struct State {
    painter: EguiPainter,
    gpu: MetalPresentation,
    input: egui_winit::State,
    app: IngeTrazoApp,
    window: std::sync::Arc<Window>,
    close_requested: bool,
    pending_textures: egui::TexturesDelta,
}
struct Host {
    state: Option<State>,
    initial_file: Option<PathBuf>,
    ctx: egui::Context,
    repaint: Instant,
    error: Option<String>,
    probe: Option<lifecycle_probe::Probe>,
}
impl Host {
    fn draw(&mut self, ev: &ActiveEventLoop) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if let Err(e) = state.gpu.poll() {
            state.app.status_message = format!("Native Metal painting paused: {e}");
            state.window.set_title(&state.app.status_message);
            self.repaint = Instant::now() + Duration::from_millis(100);
        }
        // Preserve queued raw input and texture deltas under ordinary capacity
        // backpressure; native close requests still enter application guards.
        if !state.close_requested {
            match state.gpu.can_prepare() {
                Ok(true) => {}
                Ok(false) => {
                    self.repaint = Instant::now() + Duration::from_millis(8);
                    return;
                }
                Err(e) => {
                    state.app.status_message = format!("Native Metal painting paused: {e}");
                    state.window.set_title(&state.app.status_message);
                    self.repaint = Instant::now() + Duration::from_millis(100);
                    return;
                }
            }
        }
        let mut raw = state.input.take_egui_input(&state.window);
        if state.close_requested {
            raw.viewports
                .entry(egui::ViewportId::ROOT)
                .or_default()
                .events
                .push(egui::ViewportEvent::Close);
        }
        let output = self.ctx.run(raw, |ctx| state.app.update_ui(ctx));
        if let Some(probe) = self.probe.as_mut() {
            probe.observe(&state.app, &output.shapes, state.gpu.counters().0);
        }
        state
            .input
            .handle_platform_output(&state.window, output.platform_output);
        if output
            .viewport_output
            .keys()
            .any(|id| *id != egui::ViewportId::ROOT)
        {
            self.error = Some("Native Metal secondary viewports unsupported".into());
            ev.exit();
            return;
        }
        if let Some(viewport) = output.viewport_output.get(&egui::ViewportId::ROOT) {
            let canceled = viewport
                .commands
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::CancelClose));
            if canceled {
                state.close_requested = false;
            }
            if viewport
                .commands
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Close))
                || (state.close_requested && !canceled)
            {
                ev.exit();
                return;
            }
            let mut info = egui::ViewportInfo::default();
            let mut actions = egui::ahash::HashSet::default();
            egui_winit::process_viewport_commands(
                &self.ctx,
                &mut info,
                viewport.commands.clone(),
                &state.window,
                &mut actions,
            );
            self.repaint = Instant::now()
                .checked_add(viewport.repaint_delay)
                .unwrap_or_else(|| Instant::now() + Duration::from_secs(3600));
        }
        state.pending_textures.append(output.textures_delta);
        let result = (|| -> Result<bool, String> {
            if !state.gpu.can_prepare()? {
                return Ok(false);
            }
            let primitives = self.ctx.tessellate(output.shapes, output.pixels_per_point);
            state
                .painter
                .update_textures(state.gpu.metal(), &state.pending_textures)?;
            state.pending_textures.set.clear();
            let paint =
                state
                    .painter
                    .prepare(state.gpu.metal(), &primitives, output.pixels_per_point)?;
            state.painter.free_textures(&state.pending_textures.free);
            state.pending_textures.free.clear();
            state.gpu.present_paint(paint)
        })();
        match result {
            Ok(true) => {
                if let Some(probe) = self.probe.as_mut() {
                    probe.submitted(&state.app, state.gpu.counters().0);
                }
            }
            Ok(false) => {
                self.repaint = Instant::now() + Duration::from_millis(8);
            }
            Err(e) => {
                state.app.status_message = format!("Native Metal painting paused: {e}");
                state.window.set_title(&state.app.status_message);
                eprintln!("{}", state.app.status_message);
                self.repaint = Instant::now() + Duration::from_millis(100);
            }
        }
    }
}
impl ApplicationHandler for Host {
    fn resumed(&mut self, ev: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let result = (|| -> Result<State, String> {
            let window = std::sync::Arc::new(
                ev.create_window(
                    Window::default_attributes()
                        .with_title("IngeTrazo — native Metal (opt-in)")
                        .with_inner_size(winit::dpi::LogicalSize::new(1280., 800.))
                        .with_min_inner_size(winit::dpi::LogicalSize::new(640., 480.)),
                )
                .map_err(|e| e.to_string())?,
            );
            let metal = NativeMetal::new()?;
            eprintln!(
                "Graphics backend: native Metal; device={}; unified={}; opt-in egui host",
                metal.info.name, metal.info.unified_memory
            );
            let painter = EguiPainter::new(&metal)?;
            let gpu = MetalPresentation::attach(window.clone(), metal)?;
            let input = egui_winit::State::new(
                self.ctx.clone(),
                egui::ViewportId::ROOT,
                &window,
                Some(window.scale_factor() as f32),
                window.theme(),
                Some(4096),
            );
            let mut app = IngeTrazoApp::new();
            app.system_clipboard = arboard::Clipboard::new().ok();
            app.attach_session_store(if let Some(probe) = self.probe.as_ref() {
                SessionStore::isolated(&probe.root.join("session"))
            } else {
                SessionStore::user_default()
            });
            if let Some(path) = self.initial_file.take() {
                app.open_document(&path);
            }
            if let Some(probe) = self.probe.as_mut() {
                probe.setup(&mut app)?;
            }
            Ok(State {
                painter,
                gpu,
                input,
                app,
                window,
                close_requested: false,
                pending_textures: Default::default(),
            })
        })();
        match result {
            Ok(state) => {
                state.window.request_redraw();
                self.state = Some(state);
            }
            Err(e) => {
                self.error = Some(e);
                ev.exit();
            }
        }
    }
    fn user_event(&mut self, _: &ActiveEventLoop, _: ()) {
        self.repaint = Instant::now();
        if let Some(state) = self.state.as_ref() {
            state.window.request_redraw();
        }
    }
    fn window_event(&mut self, ev: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::RedrawRequested) {
            self.draw(ev);
            return;
        }
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if let Some(probe) = self.probe.as_mut() {
            probe.native_event(&event);
        }
        let response = state.input.on_window_event(&state.window, &event);
        match event {
            WindowEvent::CloseRequested => {
                state.close_requested = true;
                self.repaint = Instant::now();
                state.window.request_redraw();
            }
            WindowEvent::Resized(size) => {
                if let Err(e) =
                    state
                        .gpu
                        .resize(size.width, size.height, state.window.scale_factor())
                {
                    self.error = Some(e);
                    ev.exit();
                }
                state.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let size = state.window.inner_size();
                if let Err(e) = state.gpu.resize(size.width, size.height, scale_factor) {
                    self.error = Some(e);
                    ev.exit();
                }
                state.window.request_redraw();
            }
            _ => {}
        }
        if response.repaint {
            self.repaint = Instant::now();
            state.window.request_redraw();
        }
    }
    fn about_to_wait(&mut self, ev: &ActiveEventLoop) {
        if let Some(state) = self.state.as_mut() {
            if let Err(e) = state.gpu.poll() {
                state.app.status_message = format!("Native Metal painting paused: {e}");
                state.window.set_title(&state.app.status_message);
                self.repaint = Instant::now() + Duration::from_millis(100);
            }
            if let Some(probe) = self.probe.as_mut() {
                if let Err(e) = probe.tick(&state.app, &state.gpu, &state.window) {
                    self.error = Some(e);
                    ev.exit();
                }
                self.repaint = self.repaint.min(Instant::now() + Duration::from_millis(16));
            }
            if Instant::now() >= self.repaint {
                state.window.request_redraw();
            }
        }
        ev.set_control_flow(ControlFlow::WaitUntil(self.repaint));
    }
    fn exiting(&mut self, _: &ActiveEventLoop) {
        if let Some(state) = self.state.take() {
            if let Some(probe) = self.probe.as_mut() {
                if let Err(e) = probe.finish(&state.app, &state.gpu, self.error.as_deref()) {
                    self.error = Some(e);
                }
            }
            eprintln!(
                "Metal egui stats {:?}; presentation {:?}",
                state.painter.stats(),
                state.gpu.counters()
            );
            drop(state);
        }
    }
}
