//! Native Metal host using the shared application and egui-winit.
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
pub(crate) mod world_viewport;
mod distance_label;
mod capture;
mod bridge_capture_probe;
mod world_app_probe;
mod world_classic_probe;
mod world_material_probe;
pub fn run_world_material_probe(root:PathBuf)->Result<(),String>{world_material_probe::run(root)}
pub fn run_world_classic_probe(root:PathBuf)->Result<(),String>{world_classic_probe::run(root)}
mod world_loose_probe;
pub fn run_world_loose_probe(root:PathBuf)->Result<(),String>{world_loose_probe::run(root)}
pub fn run_world_offscreen_probe(root:PathBuf)->Result<(),String>{world_app_probe::run(root)}
/// Engineering probe through actual own-window Cocoa events, not physical input.
pub fn run_document_probe(root: PathBuf, choice: &str) -> Result<(), Box<dyn std::error::Error>> {
    let probe = lifecycle_probe::Probe::new(root, choice)?;
    let initial = Some(probe.document.clone());
    run_inner(initial, Some(probe), HostOptions::diagnostic(), None)
}
pub fn run(initial_file: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    run_inner(initial_file, None, HostOptions::diagnostic(), None)
}
/// Production world and CAD callbacks use the same native Metal device.
pub fn run_native(initial_file: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    run_inner(initial_file, None, HostOptions { world: true, precise_cad: true }, None)
}
pub fn run_bridge_capture_probe(source:PathBuf,output:PathBuf)->Result<(),Box<dyn std::error::Error>>{
    let probe=bridge_capture_probe::Probe::new(source,output)?;
    run_inner(Some(probe.input.clone()),None,HostOptions{world:true,precise_cad:true},Some(probe))
}
struct HostOptions { world: bool, precise_cad: bool }
impl HostOptions {
    fn diagnostic() -> Self {
        Self {
            world: std::env::var_os("INGETRAZO_METAL_WORLD").is_some(),
            precise_cad: std::env::var_os("INGETRAZO_METAL_CAD_CALLBACKS").is_some(),
        }
    }
}
fn run_inner(
    initial_file: Option<PathBuf>,
    probe: Option<lifecycle_probe::Probe>,
    bridge_probe: Option<bridge_capture_probe::Probe>,
    options: HostOptions,
    bridge_probe: Option<bridge_capture_probe::Probe>,
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
        bridge_probe,
        options,
    };
    event.run_app(&mut host)?;
    if let Some(error) = host.error {
        return Err(error.into());
    }
    Ok(())
}
struct State {
    sheet_image:Option<crate::sheet_image::MetalExporter>,
    painter: EguiPainter,
    gpu: MetalPresentation,
    input: egui_winit::State,
    pointer_modifiers: crate::pointer_modifiers::Handle,
    app: IngeTrazoApp,
    window: std::sync::Arc<Window>,
    close_requested: bool,
    pending_textures: egui::TexturesDelta,
    world_submitted: bool,
    capture: capture::Owner,
    first_world_submission: Option<u64>,
}
struct Host {
    state: Option<State>,
    initial_file: Option<PathBuf>,
    ctx: egui::Context,
    repaint: Instant,
    error: Option<String>,
    probe: Option<lifecycle_probe::Probe>,
    options: HostOptions,
}
impl Host {
    fn draw(&mut self, ev: &ActiveEventLoop) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if EguiPainter::capture_admission(state.gpu.metal()).is_err(){
            state.app.ai_bridge_metal_capture_ready=false;
            state.app.poll_ai_bridge();
        }
        if let Err(e)=capture::poll(state){state.app.status_message=e;}
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
        if let Ok(mut timeline) = state.pointer_modifiers.lock() { timeline.begin_frame(); }
        if state.close_requested {
            raw.viewports
                .entry(egui::ViewportId::ROOT)
                .or_default()
                .events
                .push(egui::ViewportEvent::Close);
        }
        let output = self.ctx.run(raw, |ctx| {
            state.app.update_ui(ctx);
            if let Some(probe) = self.probe.as_ref() { probe.paint_raw_swatch(ctx); }
        });
        if let Some(request)=state.app.prepare_sheet_image_request(&self.ctx){let result=(||{let(request,plan)=request?;if state.sheet_image.is_none(){state.sheet_image=Some(crate::sheet_image::MetalExporter::new(state.gpu.shared_metal())?);}state.sheet_image.as_mut().unwrap().save(&plan,&request.path)})();state.app.sheet_image_result(result);}
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
            capture::after_ui(state, &self.ctx)?;
            let primitives = self.ctx.tessellate(output.shapes, output.pixels_per_point);
            // A freed ID used by this frame must still upload before prepare.
            // Opaque callbacks conservatively retain their IDs until preparation.
            if !state.pending_textures.free.is_empty()
                && !primitives.iter().any(|p| matches!(p.primitive, egui::epaint::Primitive::Callback(_))) {
                let used: std::collections::HashSet<_> = primitives.iter().filter_map(|p| match &p.primitive {
                    egui::epaint::Primitive::Mesh(mesh) => Some(mesh.texture_id), _ => None,
                }).collect();
                let dead = state.pending_textures.free.iter().copied().filter(|id| !used.contains(id)).collect();
                crate::paper_raw_texture::cancel_vec(&self.ctx, &dead, &mut state.pending_textures.set)?;
            }
            let mut images = Vec::new();
            for (id, _) in &state.pending_textures.set {
                if let Some(image) = crate::paper_raw_texture::peek(&self.ctx, *id)? {
                    images.push((*id, image));
                }
            }
            let encoded: Vec<_> = images.iter().map(|(id, image)| (*id, image.size, image.pixels.as_slice())).collect();
            state.painter.update_textures_encoded(state.gpu.metal(), &state.pending_textures, &encoded)?;
            // A failed transaction retains every payload/delta. ACK only after
            // complete GPU owner publication; image clones only retain Arc bytes.
            for (id, _) in &images { crate::paper_raw_texture::acknowledge(&self.ctx, *id)?; }
            state.pending_textures.set.clear();
            world_viewport::install(&mut state.painter,state.gpu.metal(),&state.app)?;

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
                if state.app.native_gpu_viewport_enabled && state.app.native_gpu_viewport_texture.is_some() && !state.world_submitted {
                    eprintln!("Graphics backend: native Metal; first GPU world frame submitted; document={:?}", state.app.current_file);
                    state.world_submitted=true;
                    state.first_world_submission=Some(state.gpu.counters().0);
                }
                if let Some(probe) = self.probe.as_mut() {
                    probe.submitted(&state.app, state.gpu.counters().0);
                    probe.canvas_submitted(&self.ctx, &state.painter);
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
                        .with_title("IngeTrazo — native Metal")
                        .with_inner_size(winit::dpi::LogicalSize::new(1280., 800.))
                        .with_min_inner_size(winit::dpi::LogicalSize::new(640., 480.)),
                )
                .map_err(|e| e.to_string())?,
            );
            let metal = NativeMetal::new()?;
            eprintln!(
                "Graphics backend: native Metal; device={}; unified={}",
                metal.info.name, metal.info.unified_memory
            );
            let precise_cad = self.options.precise_cad;
            let viewport_cad = self.options.world;
            if (precise_cad || viewport_cad) && !ingetrazo_metal::MetalCad2d::supported(&metal) && !ingetrazo_metal::cad_compute_painter::Painter::supported(&metal) {
                return Err("Native Metal exact callback requires Apple framebuffer fetch or read/write RGBA canvas; no approximate fallback".into());
            }
            let painter = if (precise_cad || viewport_cad)&&!ingetrazo_metal::MetalCad2d::supported(&metal){EguiPainter::new_compute(&metal)?}else{EguiPainter::new(&metal)?};
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
            if viewport_cad{app.native_metal_world=Some(world_viewport::Viewport::new(gpu.shared_metal()));}
            if precise_cad {
                app.native_composer.factory=Some(crate::composer_backend::Factory::MetalProducer(ingetrazo_render::cad_callback::Factory::default()));
            }
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
            if let Some(probe)=self.bridge_probe.as_mut(){probe.setup(&mut app)?;}
            crate::paper_raw_texture::install(&self.ctx);
            if let Some(probe) = self.probe.as_mut() { probe.setup_raw_swatch(&self.ctx)?; }
            let pointer_modifiers = crate::pointer_modifiers::install(&self.ctx);
            Ok(State {
                sheet_image:None,
                painter,
                gpu,
                input,
                pointer_modifiers,
                app,
                window,
                close_requested: false,
                pending_textures: Default::default(),
                world_submitted: false,
                capture: {let mut owner=capture::Owner::default();if self.bridge_probe.is_some(){owner.enable_diagnostics();}owner},
                first_world_submission: None,
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
        let appended_from = state.input.egui_input().events.len();
        let response = state.input.on_window_event(&state.window, &event);
        if let Ok(mut timeline) = state.pointer_modifiers.lock() {
            timeline.record_appended(state.input.egui_input(), appended_from);
        }
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
            if EguiPainter::capture_admission(state.gpu.metal()).is_ok() && state.first_world_submission.is_some_and(|serial|state.gpu.was_displayed(serial)) {
                state.app.ai_bridge_metal_capture_ready=true;
            }
            if let Some(probe)=self.bridge_probe.as_mut(){
                match probe.tick(state,&self.ctx){
                    Ok(true)=>ev.exit(),Ok(false)=>{},Err(e)=>{self.error=Some(e);ev.exit();}
                }
                self.repaint=self.repaint.min(Instant::now()+Duration::from_millis(8));
            }
            if let Some(probe) = self.probe.as_mut() {
                if let Err(e) = probe.tick(&state.app, &state.gpu, &state.window) {
                    self.error = Some(e);
                    ev.exit();
                }
                self.repaint = self.repaint.min(Instant::now() + Duration::from_millis(16));
            }
            if capture::outstanding(state) {
                self.repaint=self.repaint.min(Instant::now()+Duration::from_millis(8));
            }
            if Instant::now() >= self.repaint {
                state.window.request_redraw();
            }
        }
        ev.set_control_flow(ControlFlow::WaitUntil(self.repaint));
    }
    fn exiting(&mut self, _: &ActiveEventLoop) {
        if let Some(state) = self.state.take() {
            if let Some(probe)=self.bridge_probe.as_mut(){if let Err(e)=probe.finish(){self.error=Some(e);}}
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

mod world_ghost_probe;
pub fn run_world_ghost_probe(input:PathBuf,output:PathBuf)->Result<(),String>{world_ghost_probe::run(input,output)}

mod world_sweep_probe;
pub fn run_world_sweep_probe(input:PathBuf,output:PathBuf)->Result<(),String>{world_sweep_probe::run(input,output)}

mod document_annotations_probe;
pub fn run_document_annotations_probe(input:PathBuf,output:PathBuf)->Result<(),String>{document_annotations_probe::run(input,output)}

mod billboard_probe;
/// Bounded original-document billboard stage acceptance.
pub fn run_billboard_probe(input:PathBuf,output:PathBuf)->Result<(),String>{billboard_probe::run(input,output)}
