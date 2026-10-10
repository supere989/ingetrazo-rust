//! Native WSI clear/resize foundation only. No application paint proof.
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ingetrazo_metal::{NativeMetal, presentation::MetalPresentation};
    use winit::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::{Window, WindowId},
    };
    struct Probe {
        gpu: Option<MetalPresentation>,
        window: Option<std::sync::Arc<Window>>,
        error: Option<String>,
        frames: u64,
        automatic: bool,
        started: std::time::Instant,
        resized: bool,
        resize_requested: bool,
        resize_target: Option<winit::dpi::PhysicalSize<u32>>,
        diagnostics_at: std::time::Instant,
    }
    impl ApplicationHandler for Probe {
        fn resumed(&mut self, ev: &ActiveEventLoop) {
            let result = (|| -> Result<(), String> {
                let window = std::sync::Arc::new(
                    ev.create_window(
                        Window::default_attributes()
                            .with_title("IngeTrazo native Metal presentation probe")
                            .with_inner_size(winit::dpi::LogicalSize::new(800., 600.)),
                    )
                    .map_err(|e| e.to_string())?,
                );
                let metal = NativeMetal::new()?;
                eprintln!(
                    "Native Metal WSI: device={}; unified={}; FOUNDATION ONLY",
                    metal.info.name, metal.info.unified_memory
                );
                self.gpu = Some(MetalPresentation::attach(window.clone(), metal)?);
                self.window = Some(window);
                Ok(())
            })();
            if let Err(e) = result {
                self.error = Some(e);
                ev.exit();
            }
        }
        fn window_event(&mut self, ev: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            let Some(gpu) = self.gpu.as_mut() else { return };
            let result = match event {
                WindowEvent::Occluded(value) => {
                    eprintln!("WSI actual Occluded({value})");
                    Ok(())
                }
                WindowEvent::Focused(value) => {
                    eprintln!("WSI actual Focused({value})");
                    Ok(())
                }
                WindowEvent::CloseRequested => {
                    eprintln!("presentation counters {:?}", gpu.counters());
                    ev.exit();
                    Ok(())
                }
                WindowEvent::Resized(size) => {
                    if self.resize_requested && Some(size) == self.resize_target { self.resized = true; }
                    gpu.resize(
                        size.width,
                        size.height,
                        self.window.as_ref().unwrap().scale_factor(),
                    )
                }
                WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                    let s = self.window.as_ref().unwrap().inner_size();
                    gpu.resize(s.width, s.height, scale_factor)
                }
                WindowEvent::RedrawRequested => {
                    gpu.present_clear([0.08, 0.2, 0.35, 1.]).map(|submitted| {
                        if submitted {
                            self.frames += 1;
                        }
                    })
                }
                _ => Ok(()),
            };
            if let Err(e) = result {
                self.error = Some(e);
                ev.exit();
            }
        }
        fn about_to_wait(&mut self, ev: &ActiveEventLoop) {
            if let Some(gpu) = self.gpu.as_mut() {
                if let Err(e) = gpu.poll() {
                    self.error = Some(e);
                    ev.exit();
                    return;
                }
            }
            if self.automatic {
                if self.diagnostics_at.elapsed() >= std::time::Duration::from_secs(1) {
                    if let Some(gpu) = self.gpu.as_ref() {
                        eprintln!("WSI observation: {}", gpu.diagnostics());
                    }
                    self.diagnostics_at = std::time::Instant::now();
                }
                if self.started.elapsed() > std::time::Duration::from_secs(15) {
                    self.error = Some("bounded WSI probe missed presented completion".into());
                    ev.exit();
                    return;
                }
                if let Some(gpu) = self.gpu.as_ref() {
                    let (_, retired, pending) = gpu.counters();
                    if retired >= 2 && !self.resize_requested {
                        self.resize_requested = true;
                        let window = self.window.as_ref().unwrap();
                        let logical = winit::dpi::LogicalSize::new(640., 480.);
                        self.resize_target = Some(logical.to_physical(window.scale_factor()));
                        let _ = window.request_inner_size(logical);
                    }
                    if self.resized && retired >= 6 && pending == 0 {
                        eprintln!(
                            "PASS actual CAMetalLayer presented retirement and resize {:?}",
                            gpu.counters()
                        );
                        ev.exit();
                        return;
                    }
                }
            }
            // Probe cadence only; production repaint scheduling belongs to egui.
            ev.set_control_flow(ControlFlow::WaitUntil(
                std::time::Instant::now() + std::time::Duration::from_millis(16),
            ));
            if let Some(w) = self.window.as_ref() {
                if !self.automatic || self.frames < if self.resized { 6 } else { 2 } {
                    w.request_redraw();
                }
            }
        }
        fn exiting(&mut self, _: &ActiveEventLoop) {
            self.gpu.take();
            self.window.take();
        }
    }
    let event = EventLoop::new()?;
    let mut probe = Probe {
        gpu: None,
        window: None,
        error: None,
        frames: 0,
        automatic: std::env::args().any(|s| s == "--automatic"),
        started: std::time::Instant::now(),
        resized: false,
        resize_requested: false,
        resize_target: None,
        diagnostics_at: std::time::Instant::now(),
    };
    event.run_app(&mut probe)?;
    if let Some(e) = probe.error {
        return Err(e.into());
    }
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Native Metal presentation requires macOS");
    std::process::exit(2);
}
