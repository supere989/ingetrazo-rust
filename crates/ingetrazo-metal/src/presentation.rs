//! Opt-in native CAMetalLayer presentation foundation. Not an egui painter.
use crate::NativeMetal;
use objc2::{MainThreadMarker, rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::NSView;
use objc2_core_foundation::CGSize;
use objc2_metal::*;
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{collections::VecDeque, mem::ManuallyDrop, sync::atomic::Ordering};
use winit::window::Window;

fn next_terminal<T>(
    frames: &std::collections::VecDeque<T>,
    mut proof: impl FnMut(&T) -> Result<Option<bool>, String>,
) -> Result<Option<(usize, bool)>, String> {
    // Admission already limits this queue to three immutable owners. No sorting,
    // allocation or waiting is needed, and unknown owners stay in place.
    for (index, frame) in frames.iter().enumerate() {
        if let Some(displayed) = proof(frame)? {
            return Ok(Some((index, displayed)));
        }
    }
    Ok(None)
}

fn terminal_proof(
    status: MTLCommandBufferStatus,
    callback_seen: bool,
    time: f64,
) -> Result<Option<bool>, String> {
    if status != MTLCommandBufferStatus::Completed || !callback_seen {
        return Ok(None);
    }
    if !time.is_finite() || time < 0. {
        return Err("Invalid native presentation completion timestamp; ownership retained".into());
    }
    Ok(Some(time > 0.))
}
struct Frame {
    submission_id: u64,
    command: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    // Observation only; do not hold the application drawable until its callback.
    drawable_id: usize,
    _paint: Option<std::sync::Arc<dyn std::any::Any>>,
    _canvas:Option<std::sync::Arc<crate::compute_canvas::Lease>>,
    callback_seen: std::sync::Arc<std::sync::atomic::AtomicBool>,
    callback_time: std::sync::Arc<std::sync::atomic::AtomicU64>,
    _callback: block2::RcBlock<dyn Fn(std::ptr::NonNull<ProtocolObject<dyn MTLDrawable>>)>,
}
struct Owners {
    frames: VecDeque<Frame>,
    canvases:crate::compute_canvas::Pool,
    layer: Retained<CAMetalLayer>,
    // Keep winit/AppKit window/view/delegate lifetime through quarantined work.
    _window: std::sync::Arc<Window>,
    // Device and persistent queue outlive all drawable/command owners.
    metal: NativeMetal,
}
/// Retains the attached winit Window, including when GPU ownership is quarantined.
/// Unknown GPU OR presentation completion quarantines the complete owner.
pub struct MetalPresentation {
    owners: ManuallyDrop<Owners>,
    submitted: u64,
    retired: u64,
    displayed: u64,
    skipped: u64,
    displayed_submissions: [u64; 32],
    displayed_cursor: usize,
    extent: (u32, u32),
}
impl MetalPresentation {
    pub fn attach(window: std::sync::Arc<Window>, metal: NativeMetal) -> Result<Self, String> {
        let mtm = MainThreadMarker::new().ok_or("Metal window attachment requires main thread")?;
        let RawWindowHandle::AppKit(handle) =
            window.window_handle().map_err(|e| e.to_string())?.as_raw()
        else {
            return Err("Native Metal requires an AppKit window".into());
        };
        // winit guarantees a live NSView for the Window handle; this method
        // does not retain/use that view after attachment.
        let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        let _ = mtm;
        let layer = CAMetalLayer::new();
        layer.setDevice(Some(&metal.device));
        layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        layer.setFramebufferOnly(true);
        layer.setMaximumDrawableCount(3);
        // Apple bounds nextDrawable acquisition to approximately one second.
        // This is an explicit foundation latency limitation, not a fast-path claim.
        layer.setAllowsNextDrawableTimeout(true);
        layer.setContentsScale(window.scale_factor());
        // A custom CAMetalLayer is layer-hosted, not AppKit's layer-backed
        // drawing store. Apple's NSView.wantsLayer contract requires this order:
        // install the supplied layer FIRST, then opt into layer hosting.
        // https://developer.apple.com/documentation/appkit/nsview/wantslayer
        let prior_wants_layer = view.wantsLayer();
        // winit's content view can already be layer-backed. Establish the
        // documented hosting transition explicitly before installing our layer;
        // attachment precedes any GPU command/drawable ownership.
        if prior_wants_layer {
            view.setWantsLayer(false);
            if view.wantsLayer() {
                return Err("AppKit refused to leave prior layer-backed mode".into());
            }
        }
        view.setLayer(Some(&layer));
        view.setWantsLayer(true);
        let view_layer_matches = view.layer().is_some_and(|actual| {
            Retained::as_ptr(&actual).cast::<std::ffi::c_void>()
                == Retained::as_ptr(&layer).cast::<std::ffi::c_void>()
        });
        if !view_layer_matches || !view.wantsLayer() {
            return Err("AppKit did not retain the supplied Metal hosting layer".into());
        }
        eprintln!(
            "Metal layer-hosting order: backed-off-then-custom-layer-before-wantsLayer; prior_wantsLayer={prior_wants_layer}; actual_wantsLayer={}; view_layer_matches={view_layer_matches}",
            view.wantsLayer()
        );
        eprintln!(
            "Metal WSI attachment: view_bounds={:?}; layer_frame={:?}; layer_bounds={:?}; layer_hidden={}; window_visible={:?}; drawable_size={:?}",
            view.bounds(),
            layer.frame(),
            layer.bounds(),
            layer.isHidden(),
            window.is_visible(),
            layer.drawableSize()
        );
        let mut result = Self {
            owners: ManuallyDrop::new(Owners {
                frames: VecDeque::new(),
                canvases:crate::compute_canvas::Pool::default(),
                layer,
                _window: window.clone(),
                metal,
            }),
            submitted: 0,
            retired: 0,
            displayed: 0,
            skipped: 0,
            displayed_submissions: [0; 32],
            displayed_cursor: 0,
            extent: (0, 0),
        };
        let size = window.inner_size();
        result.resize(size.width, size.height, window.scale_factor())?;
        Ok(result)
    }
    pub fn resize(&mut self, width: u32, height: u32, density: f64) -> Result<(), String> {
        if !density.is_finite() || density <= 0.0 || width > 16384 || height > 16384 {
            return Err("Invalid native drawable extent/density".into());
        }
        // A supplied CAMetalLayer is layer-hosted: AppKit does not maintain its
        // presentation geometry. drawableSize only allocates pixel textures.
        // Keep its logical frame equal to the actual live content view bounds.
        MainThreadMarker::new().ok_or("Metal layer resize requires main thread")?;
        let RawWindowHandle::AppKit(handle) = self
            .owners
            ._window
            .window_handle()
            .map_err(|e| e.to_string())?
            .as_raw()
        else {
            return Err("Native Metal requires AppKit view".into());
        };
        let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        self.owners.layer.setFrame(view.bounds());
        self.extent = (width, height);
        self.owners.layer.setContentsScale(density);
        self.owners
            .layer
            .setDrawableSize(CGSize::new(width as f64, height as f64));
        Ok(())
    }
    /// Nonblocking completion query. Command completion alone is insufficient:
    /// presentation can still own the drawable after rendering has completed.
    pub fn poll(&mut self) -> Result<usize, String> {
        let mut count = 0;
        loop {
            let terminal = next_terminal(&self.owners.frames, |frame| {
                if frame.command.status() == MTLCommandBufferStatus::Error {
                    return Err("Metal presentation command failed; owners quarantined".into());
                }
                terminal_proof(
                    frame.command.status(),
                    frame.callback_seen.load(Ordering::Acquire),
                    f64::from_bits(frame.callback_time.load(Ordering::Acquire)),
                )
            });
            let (index, displayed) = match terminal {
                Ok(Some(terminal)) => terminal,
                Ok(None) => break,
                Err(error) => {
                    self.owners.metal.poisoned.store(true, Ordering::Release);
                    return Err(error);
                }
            };
            // Each immutable frame retains its command, callback and paint.
            // Metal owns scheduled presentation; the application drawable was
            // released after commit, without weakening retirement proof.
            // A missing head callback cannot invalidate a later owner's actual
            // terminal proof. Never release the unresolved head by inference.
            if displayed {
                let submission = self.owners.frames[index].submission_id;
                self.displayed_submissions[self.displayed_cursor] = submission;
                self.displayed_cursor =
                    (self.displayed_cursor + 1) % self.displayed_submissions.len();
                self.displayed += 1;
            } else {
                self.skipped += 1;
            }
            self.owners.frames.remove(index);
            self.retired += 1;
            count += 1;
        }
        Ok(count)
    }
    /// Diagnostic clear frame. This does not paint the application or CAD.
    pub fn present_clear(&mut self, rgba: [f64; 4]) -> Result<bool, String> {
        objc2::rc::autoreleasepool(|_| self.present_clear_inner(rgba))
    }
    fn present_clear_inner(&mut self, rgba: [f64; 4]) -> Result<bool, String> {
        self.present_inner(rgba, None)
    }
    fn present_inner(
        &mut self,
        rgba: [f64; 4],
        paint: Option<std::sync::Arc<crate::egui_painter::PreparedPaint>>,
    ) -> Result<bool, String> {
        self.poll()?;
        if self.owners.metal.poisoned.load(Ordering::Acquire) {
            return Err("Metal admission poisoned".into());
        }
        if rgba.iter().any(|x| !x.is_finite() || *x < 0.0 || *x > 1.0) {
            return Err("Invalid clear color".into());
        }
        if self.extent.0 == 0 || self.extent.1 == 0 || self.owners.frames.len() >= 3 {
            return Ok(false);
        }
        let canvas=if paint.as_ref().is_some_and(|p|p.requires_canvas()){
            let owners=&mut *self.owners;let Some(canvas)=owners.canvases.acquire(&owners.metal,self.extent)?else{return Ok(false)};Some(canvas)
        }else{None};
        let Some(drawable) = self.owners.layer.nextDrawable() else {
            return Ok(false);
        };
        let command = self
            .owners
            .metal
            .queue
            .commandBuffer()
            .ok_or("Metal command allocation failed")?;
        if let Some(paint)=paint.as_ref(){paint.encode_world(&command)?;}
        if let Some(canvas)=canvas.as_ref(){
            paint.as_ref().unwrap().encode_canvas(&command,&canvas,rgba)?;
            canvas.present(&command,&drawable.texture())?;
        }else{
        let desc = MTLRenderPassDescriptor::new();
        // Metal render pass guarantees eight color slots; index zero is in range.
        let attachment = unsafe { desc.colorAttachments().objectAtIndexedSubscript(0) };
        attachment.setTexture(Some(&drawable.texture()));
        attachment.setLoadAction(MTLLoadAction::Clear);
        attachment.setStoreAction(MTLStoreAction::Store);
        attachment.setClearColor(MTLClearColor {
            red: rgba[0],
            green: rgba[1],
            blue: rgba[2],
            alpha: rgba[3],
        });
        let encoder = command
            .renderCommandEncoderWithDescriptor(&desc)
            .ok_or("Metal render encoder failed")?;
        if let Some(paint) = paint.as_ref() {
            if let Err(e) = paint.encode(&encoder, self.extent) {
                encoder.endEncoding();
                return Err(e);
            }
        }
        encoder.endEncoding();
        }
        let callback_seen = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let callback_time = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let seen = callback_seen.clone();
        let time = callback_time.clone();
        let callback = block2::RcBlock::new(
            move |received: std::ptr::NonNull<ProtocolObject<dyn MTLDrawable>>| {
                // Metal guarantees its callback argument is valid during invocation.
                let drawable = unsafe { received.as_ref() };
                time.store(drawable.presentedTime().to_bits(), Ordering::Release);
                seen.store(true, Ordering::Release);
            },
        );
        unsafe {
            drawable.addPresentedHandler(block2::RcBlock::as_ptr(&callback));
        }
        let drawable_id = drawable.drawableID();
        command.presentDrawable(ProtocolObject::from_ref(&*drawable));
        command.commit();
        // Apple recommends releasing the application drawable immediately after
        // CPU frame finalization. presentDrawable schedules presentation through
        // the command buffer; no post-submit drawable queries require our own
        // strong reference. Keep every command/callback/paint owner below until
        // actual GPU completion AND terminal presentation callback.
        // https://developer.apple.com/library/archive/documentation/3DDrawing/Conceptual/MTLBestPracticesGuide/Drawables.html
        drop(drawable);
        self.owners.frames.push_back(Frame {
            submission_id: self.submitted + 1,
            command,
            drawable_id,
            callback_seen,
            callback_time,
            _callback: callback,
            _paint: paint.map(|p| p as std::sync::Arc<dyn std::any::Any>),
            _canvas:canvas,
        });
        self.submitted += 1;
        Ok(true)
    }
    /// Admission gate before any texture, mesh, or target preparation. Input
    /// remains queued in the host while drawable capacity/extent is unavailable.
    pub fn can_prepare(&mut self) -> Result<bool, String> {
        self.poll()?;
        if self.owners.metal.poisoned.load(Ordering::Acquire) {
            return Err("Metal admission poisoned".into());
        }
        Ok(self.extent.0 > 0 && self.extent.1 > 0 && self.owners.frames.len() < 3)
    }
    pub fn metal(&self) -> &NativeMetal {
        &self.owners.metal
    }
    pub fn extent(&self) -> (u32, u32) {
        self.extent
    }
    /// Caller supplies immutable owned prepared GPU paint. The complete owner
    /// remains alive until both render and presentation completion are observed.
    pub fn present_paint(
        &mut self,
        paint: std::sync::Arc<crate::egui_painter::PreparedPaint>,
    ) -> Result<bool, String> {
        objc2::rc::autoreleasepool(|_| self.present_inner([0.06, 0.06, 0.06, 1.], Some(paint)))
    }
    /// Own AppKit window identity for bounded engineering capture only. Never
    /// returns a desktop/global window ID or accesses another application's view.
    pub fn native_window_id(&self) -> Result<u32, String> {
        MainThreadMarker::new().ok_or("Native window identity requires main thread")?;
        let RawWindowHandle::AppKit(handle) = self
            .owners
            ._window
            .window_handle()
            .map_err(|e| e.to_string())?
            .as_raw()
        else {
            return Err("Not an AppKit window".into());
        };
        let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        let native = view
            .window()
            .ok_or("Own content view is not attached to window")?;
        let id =
            u32::try_from(native.windowNumber()).map_err(|_| "Own NSWindow ID out of domain")?;
        if id == 0 {
            return Err("Own NSWindow ID zero refused".into());
        }
        Ok(id)
    }
    pub fn completed_inflight_for_probe(&self) -> usize {
        self.owners
            .frames
            .iter()
            .filter(|f| f.command.status() == MTLCommandBufferStatus::Completed)
            .count()
    }
    /// Read-only native diagnostics. Completed is never substituted for displayed.
    pub fn diagnostics(&self) -> String {
        // Visibility alone does not establish that the compositor can display
        // this window. Inspect AppKit only on its owning main thread.
        let appkit = MainThreadMarker::new().and_then(|_| {
            let handle = self.owners._window.window_handle().ok()?;
            let RawWindowHandle::AppKit(handle) = handle.as_raw() else { return None; };
            let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
            let window = view.window()?;
            // Observer only: these properties do not establish presentation.
            let screen = window.screen().map(|screen| format!(
                "name={} backing_scale={} max_fps={} refresh_min={} refresh_max={} frame={:?}",
                screen.localizedName(), screen.backingScaleFactor(),
                screen.maximumFramesPerSecond(), screen.minimumRefreshInterval(),
                screen.maximumRefreshInterval(), screen.frame()));
            Some(format!("number={} visible={} active_space={} occlusion={:?} screen={:?}",
                window.windowNumber(), window.isVisible(), window.isOnActiveSpace(), window.occlusionState(), screen))
        });
        let frames: Vec<_> = self
            .owners
            .frames
            .iter()
            .map(|f| {
                format!(
                    "submission={} id={} command={:?} drawable_owner=released_after_commit gpuStart={} gpuEnd={} callback_seen={} callback_time={}",
                    f.submission_id,
                    f.drawable_id,
                    f.command.status(),
                    f.command.GPUStartTime(),
                    f.command.GPUEndTime(),
                    f.callback_seen.load(Ordering::Acquire),
                    f64::from_bits(f.callback_time.load(Ordering::Acquire))
                )
            })
            .collect();
        format!(
            "appkit={:?}; extent={:?}; visible={:?}; layer_frame={:?}; bounds={:?}; hidden={}; drawable_size={:?}; presentsWithTransaction={}; displaySyncEnabled={}; device_id={}; preferred_device={:?}; submitted={}; retired={}; displayed={}; skipped={}; frames=[{}]",
            appkit,
            self.extent,
            self.owners._window.is_visible(),
            self.owners.layer.frame(),
            self.owners.layer.bounds(),
            self.owners.layer.isHidden(),
            self.owners.layer.drawableSize(),
            self.owners.layer.presentsWithTransaction(),
            self.owners.layer.displaySyncEnabled(),
            self.owners.metal.device.registryID(),
            self.owners.layer.preferredDevice().map(|device|
                (device.registryID(), device.name().to_string())),
            self.submitted,
            self.retired,
            self.displayed,
            self.skipped,
            frames.join("; ")
        )
    }
    /// Displayed frames require actual positive callback timestamps; skipped
    /// terminal callbacks are safe owner retirement, never visible proof.
    /// Positive callback proof for an exact recent submitted command, never a
    /// count-based approximation. Fixed-size diagnostic history, no warm alloc.
    pub fn was_displayed(&self, submission: u64) -> bool {
        submission > 0 && self.displayed_submissions.contains(&submission)
    }
    pub fn presentation_counters(&self) -> (u64, u64) {
        (self.displayed, self.skipped)
    }
    pub fn counters(&self) -> (u64, u64, usize) {
        (self.submitted, self.retired, self.owners.frames.len())
    }
}
impl Drop for MetalPresentation {
    fn drop(&mut self) {
        // Never block in Drop or infer completion from timeout/window destruction.
        let _ = self.poll();
        if self.owners.frames.is_empty() {
            unsafe {
                ManuallyDrop::drop(&mut self.owners);
            }
        } else {
            self.owners.metal.poisoned.store(true, Ordering::Release);
            eprintln!(
                "Metal presentation ownership retained: {} unresolved frames",
                self.owners.frames.len()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn later_terminal_owner_does_not_release_unknown_head() {
        let mut frames = std::collections::VecDeque::from([
            (MTLCommandBufferStatus::Completed, false, 0.),
            (MTLCommandBufferStatus::Completed, true, 0.),
            (MTLCommandBufferStatus::Completed, true, 7.),
        ]);
        let mut displayed = 0;
        let mut skipped = 0;
        while let Some((index, visible)) =
            next_terminal(&frames, |&(s, seen, t)| terminal_proof(s, seen, t)).unwrap()
        {
            frames.remove(index);
            if visible {
                displayed += 1;
            } else {
                skipped += 1;
            }
        }
        assert_eq!((displayed, skipped), (1, 1));
        assert_eq!(frames.len(), 1);
        assert!(!frames.front().unwrap().1);
        // Callback before GPU completion is still no retirement proof.
        frames.push_back((MTLCommandBufferStatus::Scheduled, true, 9.));
        assert_eq!(
            next_terminal(&frames, |&(s, seen, t)| terminal_proof(s, seen, t)).unwrap(),
            None
        );
        frames.push_back((MTLCommandBufferStatus::Completed, true, f64::NAN));
        assert!(next_terminal(&frames, |&(s, seen, t)| terminal_proof(s, seen, t)).is_err());
        assert_eq!(frames.len(), 3);
    }

    #[test]
    fn gpu_completion_and_terminal_callback_are_both_required() {
        assert_eq!(
            terminal_proof(MTLCommandBufferStatus::Completed, false, 1.).unwrap(),
            None
        );
        assert_eq!(
            terminal_proof(MTLCommandBufferStatus::Scheduled, true, 1.).unwrap(),
            None
        );
        assert_eq!(
            terminal_proof(MTLCommandBufferStatus::Completed, true, 0.).unwrap(),
            Some(false)
        );
        assert_eq!(
            terminal_proof(MTLCommandBufferStatus::Completed, true, 1.).unwrap(),
            Some(true)
        );
        assert!(terminal_proof(MTLCommandBufferStatus::Completed, true, f64::NAN).is_err());
        assert!(terminal_proof(MTLCommandBufferStatus::Completed, true, -1.).is_err());
    }
}
