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
                WindowEvent::CloseRequested => {
                    eprintln!("presentation counters {:?}", gpu.counters());
                    ev.exit();
                    Ok(())
                }
                WindowEvent::Resized(size) => gpu.resize(
                    size.width,
                    size.height,
                    self.window.as_ref().unwrap().scale_factor(),
                ),
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
            // Probe cadence only; production repaint scheduling belongs to egui.
            ev.set_control_flow(ControlFlow::WaitUntil(
                std::time::Instant::now() + std::time::Duration::from_millis(16),
            ));
            if let Some(w) = self.window.as_ref() {
                w.request_redraw();
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
