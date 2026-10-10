//! Engineering controls dispatching through this application's actual Cocoa
//! window. They are native event-path diagnostics, not physical pointer proof.
use objc2::MainThreadMarker;
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType, NSView};
use objc2_core_foundation::CGPoint;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

#[derive(Clone, Copy, Debug)]
pub enum MousePhase {
    Move,
    Press,
    Release,
}

fn view(window: &Window) -> Result<&NSView, String> {
    MainThreadMarker::new().ok_or("Cocoa event dispatch requires main thread")?;
    let RawWindowHandle::AppKit(handle) =
        window.window_handle().map_err(|e| e.to_string())?.as_raw()
    else {
        return Err("Cocoa event dispatch requires this app's AppKit window".into());
    };
    // The borrowed winit Window keeps its own native view alive for this call.
    Ok(unsafe { handle.ns_view.cast::<NSView>().as_ref() })
}

/// Request closure using the actual Cocoa window delegate/winit event path.
/// Never calls application document-close handlers or creates egui RawInput.
pub fn request_close_for_probe(window: &Window) -> Result<(), String> {
    let native = view(window)?
        .window()
        .ok_or("AppKit view has no owning window")?;
    native.performClose(None);
    Ok(())
}

/// Send an actual NSEvent only to the supplied application's own NSWindow.
/// Coordinates are logical points from the top-left of its content view.
/// Callers must send press/release on separate event-loop turns and require
/// observed WindowEvents/application transitions rather than assume receipt.
pub fn mouse_for_probe(
    window: &Window,
    point: [f64; 2],
    phase: MousePhase,
    timestamp: f64,
) -> Result<(), String> {
    if point.iter().any(|v| !v.is_finite()) || !timestamp.is_finite() || timestamp < 0. {
        return Err("Nonfinite Cocoa probe event refused".into());
    }
    let view = view(window)?;
    let bounds = view.bounds();
    if point[0] < 0.
        || point[1] < 0.
        || point[0] >= bounds.size.width
        || point[1] >= bounds.size.height
    {
        return Err("Cocoa probe event outside own content bounds refused".into());
    }
    let native = view.window().ok_or("AppKit view has no owning window")?;
    let event_type = match phase {
        MousePhase::Move => NSEventType::MouseMoved,
        MousePhase::Press => NSEventType::LeftMouseDown,
        MousePhase::Release => NSEventType::LeftMouseUp,
    };
    let local = if view.isFlipped() {
        CGPoint::new(bounds.origin.x + point[0], bounds.origin.y + point[1])
    } else {
        CGPoint::new(
            bounds.origin.x + point[0],
            bounds.origin.y + bounds.size.height - point[1],
        )
    };
    // NSEvent expects base-window coordinates; honor nonzero view placement.
    let location = view.convertPoint_toView(local, None);
    let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        event_type, location, NSEventModifierFlags::empty(), timestamp,
        native.windowNumber(), None, 0, 1, if matches!(phase, MousePhase::Press) { 1. } else { 0. })
        .ok_or("Cocoa refused mouse event construction")?;
    native.sendEvent(&event);
    Ok(())
}
