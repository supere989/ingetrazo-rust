//! Diagnostic native AppKit hosting without winit eventloop/delegate/observers/window.
#[cfg(target_os = "macos")]
#[path = "support/metalkit_frame.rs"]
mod metalkit_frame;
#[cfg(target_os = "macos")]
mod native {
    use crate::metalkit_frame::{Delegate, State, ViewDelegate};
    use ingetrazo_metal::NativeMetal;
    use objc2::{
        MainThreadMarker, MainThreadOnly, class, msg_send,
        rc::{Allocated, Retained},
        runtime::ProtocolObject,
    };
    use objc2_app_kit::*;
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_foundation::NSTimer;
    use objc2_metal::*;
    use objc2_quartz_core::CAMetalLayer;
    use std::{
        cell::RefCell,
        collections::VecDeque,
        rc::Rc,
        sync::atomic::Ordering,
        time::{Duration, Instant},
    };
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::args()
            .nth(1)
            .ok_or("Usage: appkit_presenter_control report.json")?;
        let mtm = MainThreadMarker::new().ok_or("AppKit control requires main thread")?;
        let app = NSApplication::sharedApplication(mtm);
        app.setDelegate(None);
        if !app.setActivationPolicy(NSApplicationActivationPolicy::Regular) {
            return Err("AppKit activation policy refused".into());
        }
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                CGRect::new(CGPoint::new(0., 0.), CGSize::new(800., 600.)),
                NSWindowStyleMask::Titled | NSWindowStyleMask::Closable,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe {
            window.setReleasedWhenClosed(false);
        }
        let metal = NativeMetal::new()?;
        let allocated: Allocated<NSView> = unsafe { msg_send![class!(MTKView), alloc] };
        let view: Option<Retained<NSView>> = unsafe {
            msg_send![allocated,initWithFrame:window.contentView().ok_or("Native content view absent")?.bounds(),device:&*metal.device]
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
        window.setContentView(Some(&view));
        window.center();
        window.makeKeyAndOrderFront(None);
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
        let layer: Option<Retained<CAMetalLayer>> = unsafe { msg_send![&view, layer] };
        if layer
            .as_ref()
            .and_then(|l| l.device())
            .map(|d| d.registryID())
            != Some(state.borrow().metal.device.registryID())
        {
            return Err("AppKit view device mismatch".into());
        }
        let started = Instant::now();
        let done = Rc::new(RefCell::new(None::<bool>));
        let s = state.clone();
        let v = view.clone();
        let a = app.clone();
        let d = done.clone();
        let block = block2::RcBlock::new(move |_timer: std::ptr::NonNull<NSTimer>| {
            objc2::rc::autoreleasepool(|_| {
                let mut s = s.borrow_mut();
                if let Err(e) = s.poll() {
                    s.error = Some(e)
                }
                let pause = s.displayed >= 2;
                let passed = pause && s.frames.is_empty() && s.error.is_none();
                let stop =
                    passed || s.error.is_some() || started.elapsed() > Duration::from_secs(15);
                if stop && !passed && s.error.is_none() {
                    s.error = Some("AppKit strict positive presentation deadline".into())
                }
                drop(s);
                if pause || stop {
                    unsafe {
                        let _: () = msg_send![&v,setPaused:true];
                    }
                }
                if stop {
                    *d.borrow_mut() = Some(passed);
                    a.stop(None);
                    if let Some(e)=NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(NSEventType::ApplicationDefined,CGPoint::new(0.,0.),NSEventModifierFlags::empty(),0.,0,None,0,0,0){a.postEvent_atStart(&e,true)}
                }
            });
        });
        let timer =
            unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(0.016, true, &block) };
        unsafe {
            let _: () = msg_send![&view,setPaused:false];
        }
        app.run();
        timer.invalidate();
        unsafe {
            let _: () = msg_send![&view,setPaused:true];
        }
        let s = state.borrow();
        let passed = done.borrow().unwrap_or(false) && s.error.is_none();
        let frames=s.frames.iter().map(|f|serde_json::json!({"id":f.id,"status":f.command.status().0,"seen":f.seen.load(Ordering::Acquire),"time":f64::from_bits(f.time.load(Ordering::Acquire))})).collect::<Vec<_>>();
        let body = serde_json::json!({"passed":passed,"error":s.error,"scope":"Native AppKit/systemMTKView diagnostic without winit hosting or observers; no production fallback","queue_device":s.metal.queue.device().registryID(),"device_registry_id":s.metal.device.registryID(),"layer_device":layer.as_ref().and_then(|l|l.device()).map(|d|d.registryID()),"draw_calls":s.draw_calls,"submitted":s.submitted,"displayed":s.displayed,"skipped":s.skipped,"pending":s.frames.len(),"no_drawable":s.no_drawable,"frames":frames,"application_active":app.isActive(),"window_visible":window.isVisible(),"window_key":window.isKeyWindow(),"framework_timed_updates":true,"framework_current_drawable":true,"winit_hosting":false});
        let unresolved = !s.frames.is_empty();
        drop(s);
        let write = std::fs::write(path, serde_json::to_vec_pretty(&body)?);
        if unresolved {
            std::mem::forget((app, window, view, delegate, state, timer, block));
        }
        write?;
        if !passed {
            return Err("Native AppKit strict presentation gate failed".into());
        }
        Ok(())
    }
}
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    match native::run() {
        Ok(()) => Ok(()),
        Err(e) => {
            if let Some(path) = std::env::args().nth(1) {
                if !std::path::Path::new(&path).exists() {
                    let _=std::fs::write(path,serde_json::to_vec_pretty(&serde_json::json!({"passed":false,"error":e.to_string(),"scope":"Native AppKit setup failure"})).unwrap());
                }
            }
            Err(e)
        }
    }
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Native AppKit control requires macOS");
    std::process::exit(2);
}
