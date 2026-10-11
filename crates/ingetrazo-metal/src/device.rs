use ingetrazo_core::gpu_memory::MemoryBudget;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_metal::{
    MTLBuffer, MTLCommandQueue, MTLCreateSystemDefaultDevice, MTLDevice, MTLResource,
    MTLResourceOptions, MTLSamplePosition,
};
use std::{
    mem::ManuallyDrop,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
// Apple's command-line Metal device discovery requires CoreGraphics linkage.
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {}

pub struct DeviceInfo {
    pub name: String,
    pub unified_memory: bool,
    pub recommended_working_set: u64,
    pub max_buffer_length: usize,
    pub ray_tracing_advertised: bool,
    pub bc_compression_advertised: bool,
}
/// One native device and persistent command queue. Resource admission is shared
/// with Vulkan as a pure ledger, without adding a render-dispatch layer.
pub struct NativeMetal {
    pub device: Retained<ProtocolObject<dyn MTLDevice>>,
    pub queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pub info: DeviceInfo,
    pub budget: Arc<Mutex<MemoryBudget>>,
    pub(crate) poisoned: Arc<AtomicBool>,
    pub(crate) programmable_sample_positions: bool,
    pub(crate) default_sample_positions_4: Option<[MTLSamplePosition; 4]>,
}
impl NativeMetal {
    pub fn new() -> Result<Self, String> {
        let device = MTLCreateSystemDefaultDevice().ok_or("No native Metal device")?;
        let queue = device
            .newCommandQueue()
            .ok_or("Metal command queue creation failed")?;
        let recommended = device.recommendedMaxWorkingSetSize();
        if recommended == 0 {
            return Err("Metal device did not expose a working-set budget".into());
        }
        let info = DeviceInfo {
            name: device.name().to_string(),
            unified_memory: device.hasUnifiedMemory(),
            recommended_working_set: recommended,
            max_buffer_length: device.maxBufferLength(),
            ray_tracing_advertised: device.supportsRaytracing(),
            bc_compression_advertised: device.supportsBCTextureCompression(),
        };
        // Device capabilities are immutable; query once, never on the frame path.
        let programmable_sample_positions = device.areProgrammableSamplePositionsSupported();
        let default_sample_positions_4 = if device.supportsTextureSampleCount(4) {
            let mut positions = [MTLSamplePosition { x: 0., y: 0. }; 4];
            unsafe {
                device.getDefaultSamplePositions_count(std::ptr::NonNull::from(&mut positions[0]), 4);
            }
            Some(positions)
        } else {
            None
        };
        let limit = recommended / 5 * 4;
        Ok(Self {
            device,
            queue,
            info,
            budget: Arc::new(Mutex::new(MemoryBudget::new([(recommended, limit)]))),
            poisoned: Arc::new(AtomicBool::new(false)),
            programmable_sample_positions,
            default_sample_positions_4,
        })
    }
    /// Shared storage is directly CPU accessible and requires no staging copy
    /// on unified hardware. Private GPU storage is explicitly requested by the
    /// caller for discrete devices or resources with no CPU access requirement.
    pub fn buffer(&self, bytes: usize, shared: bool) -> Result<Buffer, String> {
        if self.poisoned.load(Ordering::Acquire) {
            return Err("Metal submission completion unknown; device admission poisoned".into());
        }
        if bytes == 0 || bytes > self.info.max_buffer_length {
            return Err("Metal buffer extent exceeds device limits".into());
        }
        let options = if shared {
            MTLResourceOptions::StorageModeShared
        } else {
            MTLResourceOptions::StorageModePrivate
        };
        let request = u64::try_from(bytes).map_err(|_| "Metal buffer length overflow")?;
        let ticket = self
            .budget
            .lock()
            .map_err(|_| "Metal budget poisoned")?
            .reserve(0, request)?;
        let Some(raw) = self.device.newBufferWithLength_options(bytes, options) else {
            self.budget
                .lock()
                .map_err(|_| "Metal budget poisoned")?
                .cancel(ticket)?;
            return Err("Metal buffer allocation failed".into());
        };
        let allocated = raw.allocatedSize() as u64;
        let handle = (&*raw as *const ProtocolObject<dyn MTLBuffer>).cast::<()>() as usize as u64;
        let mut ledger = self.budget.lock().map_err(|_| "Metal budget poisoned")?;
        // Metal supplies actual allocation size only after allocation. Reconcile
        // before publishing; an over-budget result drops the unpublished owner.
        if allocated != request {
            ledger.cancel(ticket)?;
            let actual = ledger.reserve(0, allocated)?;
            ledger.commit(actual, handle)?;
        } else {
            ledger.commit(ticket, handle)?;
        }
        drop(ledger);
        Ok(Buffer {
            raw: ManuallyDrop::new(raw),
            budget: self.budget.clone(),
            handle,
            shared,
        })
    }
}
/// Exclusive resource owner. Submission consumes buffers into a retained owner;
/// callers cannot write or free them while GPU completion is unknown.
pub struct Buffer {
    pub(crate) raw: ManuallyDrop<Retained<ProtocolObject<dyn MTLBuffer>>>,
    budget: Arc<Mutex<MemoryBudget>>,
    handle: u64,
    shared: bool,
}
impl Buffer {
    pub fn length(&self) -> usize {
        self.raw.length()
    }
    pub fn allocation_bytes(&self) -> u64 {
        self.raw.allocatedSize() as u64
    }
    pub fn write(&mut self, data: &[u8]) -> Result<(), String> {
        if !self.shared {
            return Err("Private Metal buffer requires an explicit GPU transfer".into());
        }
        if data.len() > self.length() {
            return Err("Metal write exceeds buffer extent".into());
        }
        // Exclusive owner access; submission retains buffers inaccessible to callers.
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr(),
                self.raw.contents().as_ptr().cast::<u8>(),
                data.len(),
            );
        }
        Ok(())
    }
}
impl Drop for Buffer {
    fn drop(&mut self) {
        // Submitted buffers are only dropped after verified completion; release actual
        // Metal allocation before removing the corresponding budget charge.
        unsafe {
            ManuallyDrop::drop(&mut self.raw);
        }
        if let Ok(mut ledger) = self.budget.lock() {
            let _ = ledger.freed(self.handle);
        }
    }
}
