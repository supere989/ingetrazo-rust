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
        display_sync: bool,
        started: std::time::Instant,
        resized: bool,
        resized_display_baseline: u64,
        resize_requested: bool,
        resize_target: Option<winit::dpi::PhysicalSize<u32>>,
        diagnostics_at: std::time::Instant,
        owned_window_path: Option<std::path::PathBuf>,
        owned_capture_ack:Option<std::path::PathBuf>,
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
                self.gpu = Some(MetalPresentation::attach_with_display_sync(window.clone(), metal, self.display_sync)?);
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
                    let target = if self.resized {
                        self.resized_display_baseline + 4
                    } else {
                        2
                    };
                    gpu.poll().and_then(|_| {
                        if self.automatic
                            && (gpu.presentation_counters().0 >= target
                                || (self.resize_requested && !self.resized))
                        {
                            return Ok(());
                        }
                        gpu.present_clear([0.08, 0.2, 0.35, 1.]).map(|submitted| {
                            if submitted {
                                self.frames += 1;
                            }
                        })
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
                if self.owned_window_path.is_some() || self.diagnostics_at.elapsed() >= std::time::Duration::from_secs(1) {
                    if let Some(gpu) = self.gpu.as_ref() {
                        if self.diagnostics_at.elapsed()>=std::time::Duration::from_secs(1){eprintln!("WSI observation: {}",gpu.diagnostics());}
                        if let Some(path) = self.owned_window_path.as_ref() {
                            let snapshot = (|| -> Result<(), String> {
                                let id = gpu.native_window_id()?;
                                let (submitted, retired, pending) = gpu.counters();
                                let (displayed, skipped) = gpu.presentation_counters();
                                let body = format!(
                                    "{{\"window_id\":{id},\"submitted\":{submitted},\"retired\":{retired},\"pending\":{pending},\"displayed\":{displayed},\"skipped\":{skipped},\"completed_inflight\":{}}}",
                                    gpu.completed_inflight_for_probe()
                                );
                                let temp = path.with_extension("tmp");
                                std::fs::write(&temp, body).map_err(|e| e.to_string())?;
                                std::fs::rename(&temp, path).map_err(|e| e.to_string())?;
                                Ok(())
                            })();
                            if let Err(error) = snapshot {
                                self.error = Some(error);
                                ev.exit();
                                return;
                            }
                        }
                    }
                    if self.diagnostics_at.elapsed()>=std::time::Duration::from_secs(1){self.diagnostics_at=std::time::Instant::now();}
                }
                if self.started.elapsed() > std::time::Duration::from_secs(15) {
                    self.error = Some("bounded WSI probe missed presented completion".into());
                    ev.exit();
                    return;
                }
                if let Some(gpu) = self.gpu.as_ref() {
                    let (_, _, pending) = gpu.counters();
                    let (displayed, skipped) = gpu.presentation_counters();
                    if displayed >= 2 && pending == 0 && !self.resize_requested {
                        self.resize_requested = true;
                        self.resized_display_baseline = displayed;
                        let window = self.window.as_ref().unwrap();
                        let logical = winit::dpi::LogicalSize::new(640., 480.);
                        self.resize_target = Some(logical.to_physical(window.scale_factor()));
                        let _ = window.request_inner_size(logical);
                    }
                    if self.resized
                        && displayed >= self.resized_display_baseline + 4
                        && displayed >= 6
                        && pending == 0
                    {
                        // Keep only this own diagnostic window alive for capture after
                        // the unchanged positive-display/resize criteria are reached.
                        // The original15s deadline still precedes this branch. Capture
                        // failure also acknowledges; it never substitutes for display.
                        if self.owned_capture_ack.as_ref().is_some_and(|path|!path.is_file()){
                            ev.set_control_flow(ControlFlow::WaitUntil(std::time::Instant::now()+std::time::Duration::from_millis(16)));
                            return;
                        }
                        eprintln!(
                            "PASS actual CAMetalLayer displayed={displayed} skipped={skipped} and exact requested resize {:?}",
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
                let target = if self.resized {
                    self.resized_display_baseline + 4
                } else {
                    2
                };
                let displayed = self
                    .gpu
                    .as_ref()
                    .map_or(0, |gpu| gpu.presentation_counters().0);
                if !self.automatic
                    || (displayed < target && !(self.resize_requested && !self.resized))
                {
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
        display_sync: !std::env::args().any(|s| s == "--diagnostic-no-vsync"),
        started: std::time::Instant::now(),
        resized: false,
        resized_display_baseline: 0,
        resize_requested: false,
        resize_target: None,
        diagnostics_at: std::time::Instant::now(),
        owned_window_path: std::env::args().find_map(|a| {
            a.strip_prefix("--owned-window-id=")
                .map(std::path::PathBuf::from)
        }),
        owned_capture_ack:std::env::args().find_map(|a|a.strip_prefix("--owned-window-ack=").map(std::path::PathBuf::from)),
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
