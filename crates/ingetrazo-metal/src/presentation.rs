//! Opt-in native CAMetalLayer presentation foundation. Not an egui painter.
use crate::NativeMetal;
use objc2::{MainThreadMarker, rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::NSView;
use objc2_core_foundation::CGSize;
use objc2_metal::*;
use objc2_quartz_core::{CAMetalLayer, CAMetalDrawable};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{collections::VecDeque, mem::ManuallyDrop, sync::atomic::Ordering};
use winit::window::Window;

struct Frame {
    command: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    drawable: Retained<ProtocolObject<dyn CAMetalDrawable>>,
}
struct Owners {
    frames: VecDeque<Frame>,
    layer: Retained<CAMetalLayer>,
    // Device and persistent queue outlive all drawable/command owners.
    metal: NativeMetal,
}
/// Caller must keep the attached winit Window alive until this owner is dropped.
/// Unknown GPU OR presentation completion quarantines the complete owner.
pub struct MetalPresentation {
    owners: ManuallyDrop<Owners>,
    submitted: u64,
    retired: u64,
    extent: (u32,u32),
}
impl MetalPresentation {
    pub fn attach(window: &Window, metal: NativeMetal) -> Result<Self,String> {
        let mtm=MainThreadMarker::new().ok_or("Metal window attachment requires main thread")?;
        let RawWindowHandle::AppKit(handle)=window.window_handle().map_err(|e|e.to_string())?.as_raw() else {
            return Err("Native Metal requires an AppKit window".into());
        };
        // winit guarantees a live NSView for the Window handle; this method
        // does not retain/use that view after attachment.
        let view=unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        let _ = mtm;
        let layer=CAMetalLayer::new();
        layer.setDevice(Some(&metal.device));
        layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        layer.setFramebufferOnly(true);
        layer.setMaximumDrawableCount(3);
        // Apple bounds nextDrawable acquisition to approximately one second.
        // This is an explicit foundation latency limitation, not a fast-path claim.
        layer.setAllowsNextDrawableTimeout(true);
        layer.setContentsScale(window.scale_factor());
        view.setWantsLayer(true);
        view.setLayer(Some(&layer));
        let mut result=Self {owners:ManuallyDrop::new(Owners {frames:VecDeque::new(),layer,metal}),submitted:0,retired:0,extent:(0,0)};
        let size=window.inner_size();result.resize(size.width,size.height,window.scale_factor())?;
        Ok(result)
    }
    pub fn resize(&mut self,width:u32,height:u32,density:f64)->Result<(),String> {
        if !density.is_finite()||density<=0.0||width>16384||height>16384 {return Err("Invalid native drawable extent/density".into());}
        self.extent=(width,height);
        self.owners.layer.setContentsScale(density);
        self.owners.layer.setDrawableSize(CGSize::new(width as f64,height as f64));
        Ok(())
    }
    /// Nonblocking completion query. Command completion alone is insufficient:
    /// presentation can still own the drawable after rendering has completed.
    pub fn poll(&mut self)->Result<usize,String> {
        let mut count=0;
        while let Some(frame)=self.owners.frames.front() {
            if frame.command.status()==MTLCommandBufferStatus::Error {
                self.owners.metal.poisoned.store(true,Ordering::Release);
                return Err("Metal presentation command failed; owners quarantined".into());
            }
            if frame.command.status()!=MTLCommandBufferStatus::Completed || frame.drawable.presentedTime()<=0.0 {break;}
            self.owners.frames.pop_front();self.retired+=1;count+=1;
        }
        Ok(count)
    }
    /// Diagnostic clear frame. This does not paint the application or CAD.
    pub fn present_clear(&mut self,rgba:[f64;4])->Result<bool,String> {
        objc2::rc::autoreleasepool(|_|self.present_clear_inner(rgba))
    }
    fn present_clear_inner(&mut self,rgba:[f64;4])->Result<bool,String> {
        self.poll()?;
        if self.owners.metal.poisoned.load(Ordering::Acquire) {return Err("Metal admission poisoned".into());}
        if rgba.iter().any(|x|!x.is_finite()||*x<0.0||*x>1.0) {return Err("Invalid clear color".into());}
        if self.extent.0==0||self.extent.1==0||self.owners.frames.len()>=3 {return Ok(false);}
        let Some(drawable)=self.owners.layer.nextDrawable() else {return Ok(false);};
        let command=self.owners.metal.queue.commandBuffer().ok_or("Metal command allocation failed")?;
        let desc=MTLRenderPassDescriptor::new();
        // Metal render pass guarantees eight color slots; index zero is in range.
        let attachment=unsafe {desc.colorAttachments().objectAtIndexedSubscript(0)};
        attachment.setTexture(Some(&drawable.texture()));
        attachment.setLoadAction(MTLLoadAction::Clear);
        attachment.setStoreAction(MTLStoreAction::Store);
        attachment.setClearColor(MTLClearColor {red:rgba[0],green:rgba[1],blue:rgba[2],alpha:rgba[3]});
        let encoder=command.renderCommandEncoderWithDescriptor(&desc).ok_or("Metal render encoder failed")?;
        encoder.endEncoding();
        command.presentDrawable(ProtocolObject::from_ref(&*drawable));
        command.commit();
        self.owners.frames.push_back(Frame {command,drawable});self.submitted+=1;
        Ok(true)
    }
    pub fn counters(&self)->(u64,u64,usize) {(self.submitted,self.retired,self.owners.frames.len())}
}
impl Drop for MetalPresentation {
    fn drop(&mut self) {
        // Never block in Drop or infer completion from timeout/window destruction.
        let _=self.poll();
        if self.owners.frames.is_empty() {unsafe {ManuallyDrop::drop(&mut self.owners);}}
        else {self.owners.metal.poisoned.store(true,Ordering::Release);eprintln!("Metal presentation ownership retained: {} unresolved frames",self.owners.frames.len());}
    }
}
