//! Same-session diagnostic of the unchanged production custom presenter.
#[cfg(target_os = "macos")]
mod native {
    use ingetrazo_metal::{NativeMetal, presentation::MetalPresentation};
    use objc2_metal::{MTLCommandQueue, MTLDevice};
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    use winit::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::{Window, WindowId},
    };
    struct Probe {
        presenter: Option<MetalPresentation>,
        started: Instant,
        path: std::path::PathBuf,
        error: Option<String>,
        passed: bool,
    }
    impl Probe {
        fn finish(&mut self, ev: &ActiveEventLoop) {
            let body = if let Some(p) = &self.presenter {
                let (submitted, retired, pending) = p.counters();
                let (displayed, skipped) = p.presentation_counters();
                serde_json::json!({"passed":self.passed,"error":self.error,"scope":"Production custom direct-clear presenter diagnostic only","device":p.metal().info.name,"device_registry_id":p.metal().device.registryID(),"queue_device":p.metal().queue.device().registryID(),"submitted":submitted,"retired":retired,"pending":pending,"displayed":displayed,"skipped":skipped,"diagnostics":p.diagnostics()})
            } else {
                serde_json::json!({"passed":false,"error":self.error})
            };
            if let Err(e) = std::fs::write(&self.path, serde_json::to_vec_pretty(&body).unwrap()) {
                self.error = Some(e.to_string());
            }
            ev.exit();
        }
    }
    impl ApplicationHandler for Probe {
        fn resumed(&mut self, ev: &ActiveEventLoop) {
            let result = (|| -> Result<MetalPresentation, String> {
                let window = Arc::new(
                    ev.create_window(
                        Window::default_attributes()
                            .with_title("IngeTrazo custom presenter diagnostic")
                            .with_inner_size(winit::dpi::LogicalSize::new(800., 600.)),
                    )
                    .map_err(|e| e.to_string())?,
                );
                MetalPresentation::attach(window, NativeMetal::new()?)
            })();
            match result {
                Ok(p) => self.presenter = Some(p),
                Err(e) => {
                    self.error = Some(e);
                    self.finish(ev)
                }
            }
        }
        fn window_event(&mut self, ev: &ActiveEventLoop, _id: WindowId, e: WindowEvent) {
            if matches!(e, WindowEvent::CloseRequested) {
                self.error = Some("Custom window closed before proof".into());
                self.finish(ev)
            }
        }
        fn about_to_wait(&mut self, ev: &ActiveEventLoop) {
            let Some(p) = self.presenter.as_mut() else {
                return;
            };
            if let Err(e) = p.poll() {
                self.error = Some(e);
            }
            let (displayed, _) = p.presentation_counters();
            if displayed < 2 && self.error.is_none() {
                if let Err(e) = p.present_clear([0.08, 0.2, 0.35, 1.]) {
                    self.error = Some(e);
                }
            }
            self.passed = displayed >= 2 && p.counters().2 == 0 && self.error.is_none();
            if self.passed || self.error.is_some() {
                self.finish(ev)
            } else if self.started.elapsed() > Duration::from_secs(15) {
                self.error = Some("Custom strict positive presentation deadline".into());
                self.finish(ev)
            } else {
                ev.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + Duration::from_millis(16),
                ));
            }
        }
    }
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::args()
            .nth(1)
            .ok_or("Usage: custom_presenter_control report.json")?;
        let ev = EventLoop::new()?;
        let mut p = Probe {
            presenter: None,
            started: Instant::now(),
            path: path.into(),
            error: None,
            passed: false,
        };
        ev.run_app(&mut p)?;
        if !p.passed || p.error.is_some() {
            return Err(p
                .error
                .unwrap_or_else(|| "Custom presentation not proven".into())
                .into());
        }
        Ok(())
    }
}
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    native::run()
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Native custom diagnostic requires macOS");
    std::process::exit(2);
}
