//! Shared diagnostic draw/callback owner; host controls differ, GPU contract identical.
use ingetrazo_metal::NativeMetal;
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, extern_protocol, msg_send,
    rc::Retained,
    runtime::{NSObject, NSObjectProtocol, ProtocolObject},
};
use objc2_app_kit::NSView;
use objc2_core_foundation::CGSize;
use objc2_metal::*;
use objc2_quartz_core::CAMetalDrawable;
use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
#[link(name = "MetalKit", kind = "framework")]
unsafe extern "C" {}
extern_protocol!(
    #[name = "MTKViewDelegate"]
    pub(crate) unsafe trait ViewDelegate: NSObjectProtocol {
        #[unsafe(method(drawInMTKView:))]
        fn draw_view(&self, view: &NSView);
        #[unsafe(method(mtkView:drawableSizeWillChange:))]
        fn resize_view(&self, view: &NSView, size: CGSize);
    }
);
pub(crate) struct Frame {
    pub(crate) id: usize,
    pub(crate) command: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    pub(crate) seen: Arc<AtomicBool>,
    pub(crate) time: Arc<AtomicU64>,
    _block: block2::RcBlock<dyn Fn(std::ptr::NonNull<ProtocolObject<dyn MTLDrawable>>)>,
}
pub(crate) struct State {
    pub(crate) metal: NativeMetal,
    pub(crate) frames: VecDeque<Frame>,
    pub(crate) draw_calls: u64,
    pub(crate) submitted: u64,
    pub(crate) displayed: u64,
    pub(crate) skipped: u64,
    pub(crate) no_drawable: u64,
    pub(crate) error: Option<String>,
}
impl State {
    pub(crate) fn poll(&mut self) -> Result<(), String> {
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
                    return Err("Invalid framework presentation timestamp; owners retained".into());
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
pub(crate) struct DelegateIvars {
    state: Rc<RefCell<State>>,
}
define_class!(
    #[unsafe(super=NSObject)]
    #[thread_kind=MainThreadOnly]
    #[ivars=DelegateIvars]
    #[name = "IngeTrazoMetalKitPresenterControlDelegate"]
    pub(crate) struct Delegate;
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
    pub(crate) fn new(mtm: MainThreadMarker, state: Rc<RefCell<State>>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DelegateIvars { state });
        unsafe { msg_send![super(this), init] }
    }
}
