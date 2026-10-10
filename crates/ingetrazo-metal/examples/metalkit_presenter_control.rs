//! Diagnostic-only native MTKView lifecycle control. Never a production fallback.
#[cfg(target_os = "macos")]
mod native {
    use ingetrazo_metal::NativeMetal;
    use objc2::{
        DefinedClass, MainThreadMarker, MainThreadOnly, class, define_class, extern_protocol,
        msg_send,
        rc::{Allocated, Retained},
        runtime::{NSObject, NSObjectProtocol, ProtocolObject},
    };
    use objc2_app_kit::NSView;
    use objc2_core_foundation::CGSize;
    use objc2_metal::*;
    use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::{
        cell::RefCell,
        collections::VecDeque,
        rc::Rc,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU64, Ordering},
        },
        time::{Duration, Instant},
    };
    use winit::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::{Window, WindowId},
    };
    // Load the real system class/protocol; no emulated wrapper or replacement API.
    #[link(name = "MetalKit", kind = "framework")]
    unsafe extern "C" {}
    extern_protocol!(
        #[name = "MTKViewDelegate"]
        unsafe trait ViewDelegate: NSObjectProtocol {
            #[unsafe(method(drawInMTKView:))]
            fn draw_view(&self, view: &NSView);
            #[unsafe(method(mtkView:drawableSizeWillChange:))]
            fn resize_view(&self, view: &NSView, size: CGSize);
        }
    );
    struct Frame {
        id: usize,
        command: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
        seen: Arc<AtomicBool>,
        time: Arc<AtomicU64>,
        _block: block2::RcBlock<dyn Fn(std::ptr::NonNull<ProtocolObject<dyn MTLDrawable>>)>,
    }
    struct State {
        metal: NativeMetal,
        frames: VecDeque<Frame>,
        draw_calls: u64,
        submitted: u64,
        displayed: u64,
        skipped: u64,
        no_drawable: u64,
        error: Option<String>,
    }
    impl State {
        fn poll(&mut self) -> Result<(), String> {
            let mut i = 0;
            while i < self.frames.len() {
                let f = &self.frames[i];
                let status = f.command.status();
                if status == MTLCommandBufferStatus::Error {
                    return Err("Framework command failed; owners retained".into());
                }
                if status == MTLCommandBufferStatus::Completed && f.seen.load(Ordering::Acquire) {
                    let t = f64::from_bits(f.time.load(Ordering::Acquire));
                    if !t.is_finite() || t < 0. {
                        return Err(
                            "Invalid framework presentation timestamp; owners retained".into()
                        );
                    }
                    if t > 0. {
                        self.displayed += 1
                    } else {
                        self.skipped += 1
                    }
                    self.frames.remove(i);
                } else {
                    i += 1
                }
            }
            Ok(())
        }
        fn draw(&mut self, view: &NSView) -> Result<(), String> {
            self.draw_calls += 1;
            self.poll()?;
            if self.frames.len() >= 3 || self.error.is_some() {
                return Ok(());
            }
            // MTKView itself vends the drawable/render pass and schedules delegate draws.
            let desc: Option<Retained<MTLRenderPassDescriptor>> =
                unsafe { msg_send![view, currentRenderPassDescriptor] };
            let Some(desc) = desc else {
                self.no_drawable += 1;
                return Ok(());
            };
            let drawable: Option<Retained<ProtocolObject<dyn CAMetalDrawable>>> =
                unsafe { msg_send![view, currentDrawable] };
            let drawable = drawable.ok_or("MetalKit descriptor lacks currentDrawable")?;
            let attachment = unsafe { desc.colorAttachments().objectAtIndexedSubscript(0) };
            attachment.setLoadAction(MTLLoadAction::Clear);
            attachment.setStoreAction(MTLStoreAction::Store);
            attachment.setClearColor(MTLClearColor {
                red: 0.08,
                green: 0.2,
                blue: 0.35,
                alpha: 1.,
            });
            let command = self
                .metal
                .queue
                .commandBuffer()
                .ok_or("Framework command allocation failed")?;
            let encoder = command
                .renderCommandEncoderWithDescriptor(&desc)
                .ok_or("Framework encoder failed")?;
            encoder.endEncoding();
            let seen = Arc::new(AtomicBool::new(false));
            let time = Arc::new(AtomicU64::new(0));
            let a = seen.clone();
            let b = time.clone();
            let block = block2::RcBlock::new(
                move |p: std::ptr::NonNull<ProtocolObject<dyn MTLDrawable>>| {
                    let d = unsafe { p.as_ref() };
                    b.store(d.presentedTime().to_bits(), Ordering::Release);
                    a.store(true, Ordering::Release);
                },
            );
            unsafe {
                drawable.addPresentedHandler(block2::RcBlock::as_ptr(&block));
            }
            let id = drawable.drawableID();
            command.presentDrawable(ProtocolObject::from_ref(&*drawable));
            command.commit();
            drop(drawable);
            self.frames.push_back(Frame {
                id,
                command,
                seen,
                time,
                _block: block,
            });
            self.submitted += 1;
            Ok(())
        }
    }
    struct DelegateIvars {
        state: Rc<RefCell<State>>,
    }
    define_class!(
        #[unsafe(super=NSObject)]
        #[thread_kind=MainThreadOnly]
        #[ivars=DelegateIvars]
        #[name = "IngeTrazoMetalKitPresenterControlDelegate"]
        struct Delegate;
        unsafe impl NSObjectProtocol for Delegate {}
        unsafe impl ViewDelegate for Delegate {
            #[unsafe(method(drawInMTKView:))]
            fn draw_view(&self, view: &NSView) {
                objc2::rc::autoreleasepool(|_| {
                    let mut s = self.ivars().state.borrow_mut();
                    if let Err(e) = s.draw(view) {
                        s.error = Some(e)
                    }
                });
            }
            #[unsafe(method(mtkView:drawableSizeWillChange:))]
            fn resize_view(&self, _view: &NSView, _size: CGSize) {}
        }
    );
    impl Delegate {
        fn new(mtm: MainThreadMarker, state: Rc<RefCell<State>>) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(DelegateIvars { state });
            unsafe { msg_send![super(this), init] }
        }
    }
    struct Owners {
        window: Arc<Window>,
        view: Retained<NSView>,
        _delegate: Retained<Delegate>,
        state: Rc<RefCell<State>>,
    }
    struct Probe {
        owners: Option<Owners>,
        started: Instant,
        path: std::path::PathBuf,
        error: Option<String>,
        passed: bool,
    }
    impl Probe {
        fn finish(&mut self, ev: &ActiveEventLoop) {
            let body = if let Some(o) = self.owners.as_ref() {
                let s = o.state.borrow();
                let frames=s.frames.iter().map(|f|serde_json::json!({"id":f.id,"status":f.command.status().0,"seen":f.seen.load(Ordering::Acquire),"time":f64::from_bits(f.time.load(Ordering::Acquire))})).collect::<Vec<_>>();
                let layer: Option<Retained<CAMetalLayer>> = unsafe { msg_send![&o.view, layer] };
                serde_json::json!({"passed":self.passed,"scope":"Native MTKView hosting/timed delegate control; no production or physical-Mac acceptance","device":s.metal.info.name,"device_registry_id":s.metal.device.registryID(),"queue_device":s.metal.queue.device().registryID(),"draw_calls":s.draw_calls,"submitted":s.submitted,"displayed":s.displayed,"skipped":s.skipped,"pending":s.frames.len(),"no_drawable":s.no_drawable,"error":self.error.as_ref().or(s.error.as_ref()),"framework_timed_updates":true,"framework_current_drawable":true,"window_visible":o.window.is_visible(),"layer_device":layer.as_ref().and_then(|l|l.device()).map(|d|d.registryID()),"frames":frames})
            } else {
                serde_json::json!({"passed":false,"scope":"Native MTKView setup failure; no presentation proof","error":self.error})
            };
            if let Some(parent) = self.path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::write(&self.path, serde_json::to_vec_pretty(&body).unwrap()) {
                self.error = Some(e.to_string())
            }
            ev.exit();
        }
    }
    impl ApplicationHandler for Probe {
        fn resumed(&mut self, ev: &ActiveEventLoop) {
            let result = (|| -> Result<Owners, String> {
                let mtm = MainThreadMarker::new().ok_or("Framework setup requires main thread")?;
                let window = Arc::new(
                    ev.create_window(
                        Window::default_attributes()
                            .with_title("IngeTrazo MTKView diagnostic control")
                            .with_inner_size(winit::dpi::LogicalSize::new(800., 600.)),
                    )
                    .map_err(|e| e.to_string())?,
                );
                let RawWindowHandle::AppKit(h) =
                    window.window_handle().map_err(|e| e.to_string())?.as_raw()
                else {
                    return Err("Framework control requires AppKit window".into());
                };
                let original = unsafe { h.ns_view.cast::<NSView>().as_ref() };
                let nswindow = original.window().ok_or("AppKit content window absent")?;
                let metal = NativeMetal::new()?;
                let allocated: Allocated<NSView> = unsafe { msg_send![class!(MTKView), alloc] };
                let view: Option<Retained<NSView>> = unsafe {
                    msg_send![allocated,initWithFrame:original.bounds(),device:&*metal.device]
                };
                let view = view.ok_or("Native MTKView initialization failed")?;
                let state = Rc::new(RefCell::new(State {
                    metal,
                    frames: VecDeque::new(),
                    draw_calls: 0,
                    submitted: 0,
                    displayed: 0,
                    skipped: 0,
                    no_drawable: 0,
                    error: None,
                }));
                let delegate = Delegate::new(mtm, state.clone());
                unsafe {
                    let _: () = msg_send![&view,setColorPixelFormat:MTLPixelFormat::BGRA8Unorm];
                    let _: () = msg_send![&view,setFramebufferOnly:true];
                    let _: () = msg_send![&view,setPreferredFramesPerSecond:60isize];
                    let _: () = msg_send![&view,setEnableSetNeedsDisplay:false];
                    let _: () = msg_send![&view,setPaused:true];
                    let _: () = msg_send![&view,setDelegate:ProtocolObject::<dyn ViewDelegate>::from_ref(&*delegate)];
                }
                nswindow.setContentView(Some(&view));
                let layer: Option<Retained<CAMetalLayer>> = unsafe { msg_send![&view, layer] };
                if layer
                    .as_ref()
                    .and_then(|l| l.device())
                    .map(|d| d.registryID())
                    != Some(state.borrow().metal.device.registryID())
                {
                    return Err("Framework view does not use the requested native device".into());
                }
                Ok(Owners {
                    window,
                    view,
                    _delegate: delegate,
                    state,
                })
            })();
            match result {
                Ok(o) => {
                    self.owners = Some(o);
                    // Publish complete owners before enabling framework callbacks.
                    unsafe {
                        let _: () = msg_send![&self.owners.as_ref().unwrap().view,setPaused:false];
                    }
                }
                Err(e) => {
                    self.error = Some(e);
                    self.finish(ev)
                }
            }
        }
        fn window_event(&mut self, ev: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
            if matches!(event, WindowEvent::CloseRequested) {
                self.error = Some("Framework window closed before proof".into());
                self.finish(ev);
            }
        }
        fn about_to_wait(&mut self, ev: &ActiveEventLoop) {
            let Some(o) = self.owners.as_ref() else {
                return;
            };
            let mut s = o.state.borrow_mut();
            if let Err(e) = s.poll() {
                s.error = Some(e)
            }
            let pause = s.displayed >= 2;
            let passed = pause && s.frames.is_empty() && s.error.is_none();
            let error = s.error.clone();
            drop(s);
            if pause {
                unsafe {
                    let _: () = msg_send![&o.view,setPaused:true];
                }
            }
            if passed {
                self.passed = true;
                self.finish(ev)
            } else if let Some(e) = error {
                self.error = Some(e);
                self.finish(ev)
            } else if self.started.elapsed() > Duration::from_secs(15) {
                self.error = Some("Framework strict positive presentation deadline".into());
                self.finish(ev)
            } else {
                ev.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + Duration::from_millis(16),
                ));
            }
        }
        fn exiting(&mut self, _ev: &ActiveEventLoop) {
            if let Some(o) = self.owners.take() {
                unsafe {
                    let _: () = msg_send![&o.view,setPaused:true];
                }
                let unresolved = !o.state.borrow().frames.is_empty();
                if unresolved {
                    eprintln!(
                        "Framework control quarantines unresolved command/callback/view/window/device owners"
                    );
                    std::mem::forget(o)
                }
            }
        }
    }
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::args()
            .nth(1)
            .ok_or("Usage: metalkit_presenter_control report.json")?;
        let ev = EventLoop::new()?;
        let mut p = Probe {
            owners: None,
            started: Instant::now(),
            path: path.into(),
            error: None,
            passed: false,
        };
        ev.run_app(&mut p)?;
        if !p.passed || p.error.is_some() {
            return Err(p
                .error
                .unwrap_or_else(|| "Framework control did not prove presentation".into())
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
    eprintln!("Native MTKView diagnostic requires macOS");
    std::process::exit(2);
}
