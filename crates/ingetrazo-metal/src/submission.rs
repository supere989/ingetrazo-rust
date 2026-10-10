//! Bounded native command submission with exclusive GPU resource ownership.
use crate::{Buffer, NativeMetal};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::NSRange;
use objc2_metal::{
    MTLBlitCommandEncoder, MTLBuffer, MTLCommandBuffer, MTLCommandBufferStatus, MTLCommandEncoder,
    MTLCommandQueue, MTLDevice,
};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

/// Retains device/queue, command and every referenced allocation until actual
/// completion. Dropping an unknown submission intentionally quarantines all
/// owners and accounting for process lifetime; it never frees pending work.
pub struct Submission {
    command: Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>>,
    device: Option<Retained<ProtocolObject<dyn MTLDevice>>>,
    queue: Option<Retained<ProtocolObject<dyn MTLCommandQueue>>>,
    buffers: Vec<Buffer>,
    poisoned: std::sync::Arc<std::sync::atomic::AtomicBool>,
    completed: bool,
}
impl NativeMetal {
    /// GPU fills private storage then BLITs into shared readback storage. No
    /// CPU initialization can fabricate the returned GPU-produced bytes.
    pub fn fill_readback(&self, bytes: usize, value: u8) -> Result<Submission, String> {
        if self.poisoned.load(Ordering::Acquire) {
            return Err("Metal admission poisoned".into());
        }
        if bytes == 0 || bytes % 4 != 0 {
            return Err("Metal BLIT fill extent must be nonzero and four-byte aligned".into());
        }
        let private = self.buffer(bytes, false)?;
        let shared = self.buffer(bytes, true)?;
        let command = self
            .queue
            .commandBuffer()
            .ok_or("Metal command buffer creation failed")?;
        let encoder = command
            .blitCommandEncoder()
            .ok_or("Metal BLIT encoder creation failed")?;
        encoder.fillBuffer_range_value(&private.raw, NSRange::new(0, bytes), value);
        // Allocations are distinct, offsets/extent in range; exclusive owners
        // become inaccessible to callers before commit.
        unsafe {
            encoder.copyFromBuffer_sourceOffset_toBuffer_destinationOffset_size(
                &private.raw,
                0,
                &shared.raw,
                0,
                bytes,
            );
        }
        encoder.endEncoding();
        let submission = Submission {
            command: Some(command),
            device: Some(self.device.clone()),
            queue: Some(self.queue.clone()),
            buffers: vec![private, shared],
            poisoned: self.poisoned.clone(),
            completed: false,
        };
        submission.command.as_ref().unwrap().commit();
        Ok(submission)
    }
}
impl Submission {
    /// Zero timeout deliberately retains unknown ownership without polling;
    /// this engineering control does not fabricate a stalled GPU submission.
    /// An ordinary deadline is finite and at most five seconds.
    pub fn wait(&mut self, timeout: Duration) -> Result<bool, String> {
        if self.completed {
            return Ok(true);
        }
        if timeout > Duration::from_secs(5) {
            return Err("Metal wait exceeds five-second bound".into());
        }
        let start = Instant::now();
        loop {
            if start.elapsed() >= timeout {
                self.poisoned.store(true, Ordering::Release);
                return Ok(false);
            }
            let status = self.command.as_ref().unwrap().status();
            if status == MTLCommandBufferStatus::Completed {
                self.completed = true;
                return Ok(true);
            }
            if status == MTLCommandBufferStatus::Error {
                self.poisoned.store(true, Ordering::Release);
                return Err(format!(
                    "Metal command error; owners quarantined: {:?}",
                    self.command.as_ref().unwrap().error()
                ));
            }
            std::thread::sleep(
                Duration::from_millis(1).min(timeout.saturating_sub(start.elapsed())),
            );
        }
    }
    pub fn verify_readback(&self, value: u8) -> Result<usize, String> {
        if !self.completed {
            return Err("Metal readback before actual completion refused".into());
        }
        let buffer = &self.buffers[1];
        // Shared memory visibility is established by Completed, and no command
        // can reference this privately owned resource again.
        let bytes = unsafe {
            std::slice::from_raw_parts(buffer.raw.contents().as_ptr().cast::<u8>(), buffer.length())
        };
        if let Some((index, actual)) = bytes.iter().enumerate().find(|(_, b)| **b != value) {
            return Err(format!("GPU readback mismatch at {index}: {actual}"));
        }
        Ok(bytes.len())
    }
}
impl Drop for Submission {
    fn drop(&mut self) {
        if !self.completed {
            self.poisoned.store(true, Ordering::Release);
            // Never query status in Drop or wait indefinitely. Completion was
            // not acknowledged, so retain all allocations and their ledger.
            for buffer in self.buffers.drain(..) {
                std::mem::forget(buffer);
            }
            if let Some(command) = self.command.take() {
                std::mem::forget(command);
            }
            if let Some(queue) = self.queue.take() {
                std::mem::forget(queue);
            }
            if let Some(device) = self.device.take() {
                std::mem::forget(device);
            }
        }
    }
}
