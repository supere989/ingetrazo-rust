//! Diagnostic-only separately compiled vertex replay, never production shader identity.
use super::*;
use objc2_foundation::NSString;
#[repr(C)]
#[derive(Clone, Copy)]
struct Record {
    world: [f32; 4],
    gl_clip: [f32; 4],
    metal_clip: [f32; 4],
    distance: [f32; 4],
}
struct Flight {
    poison: Arc<std::sync::atomic::AtomicBool>,
    command: ManuallyDrop<Retained<ProtocolObject<dyn MTLCommandBuffer>>>,
    frame: ManuallyDrop<Arc<Prepared>>,
    output: ManuallyDrop<Buffer>,
    pipeline: ManuallyDrop<Retained<ProtocolObject<dyn MTLRenderPipelineState>>>,
}
impl Drop for Flight {
    fn drop(&mut self) {
        if self.command.status() == MTLCommandBufferStatus::Completed {
            unsafe {
                ManuallyDrop::drop(&mut self.output);
                ManuallyDrop::drop(&mut self.pipeline);
                ManuallyDrop::drop(&mut self.frame);
                ManuallyDrop::drop(&mut self.command);
            }
        } else {
            self.poison
                .store(true, std::sync::atomic::Ordering::Release);
            eprintln!(
                "Vertex replay completion unknown; command/frame/output/pipeline quarantined"
            );
        }
    }
}
fn read<T: Copy>(buffer: &Buffer, index: usize) -> Result<T, String> {
    if buffer.raw.storageMode() != MTLStorageMode::Shared {
        return Err("Vertex replay input must be actual Shared upload".into());
    }
    let size = std::mem::size_of::<T>();
    let offset = index
        .checked_mul(size)
        .filter(|o| {
            o.checked_add(size)
                .is_some_and(|end| end <= buffer.length())
        })
        .ok_or("Vertex replay input record outside buffer")?;
    Ok(unsafe {
        buffer
            .raw
            .contents()
            .as_ptr()
            .cast::<u8>()
            .add(offset)
            .cast::<T>()
            .read_unaligned()
    })
}
impl Prepared {
    /// Explicit probe API. Returns separate compiled vertex-stage observations, not rasterizing VS identity.
    pub fn observe_compiled_vertex_replay(
        self: &Arc<Self>,
        metal: &NativeMetal,
    ) -> Result<serde_json::Value, String> {
        self.owner.validate(metal)?;
        if self.diagnostic_positive_depth {
            return Err("Vertex replay requires untouched normal shader mode".into());
        }
        let domain = positive_depth_probe::validate(self)?;
        let placements = self
            .placements
            .as_ref()
            .ok_or("Vertex replay placement absent")?;
        let mesh = placements.meshes[0]
            .as_ref()
            .ok_or("Vertex replay mesh absent")?;
        let vertices = mesh
            .vertices
            .as_ref()
            .ok_or("Vertex replay vertices absent")?;
        let indices = mesh
            .indices
            .as_ref()
            .ok_or("Vertex replay indices absent")?;
        if vertices.length() != 60 * std::mem::size_of::<ingetrazo_render::buffer::Vertex>()
            || indices.length() != 60 * 4
        {
            return Err("Vertex replay literal60 record extent mismatch".into());
        }
        let models = self
            .target
            .instance_matrices
            .as_ref()
            .ok_or("Vertex replay models absent")?;
        let mut uploads = Vec::with_capacity(60);
        let mut index_rows = Vec::with_capacity(60);
        for id in 0..60 {
            let v: ingetrazo_render::buffer::Vertex = read(vertices, id)?;
            let index: u32 = read(indices, id)?;
            if index >= 60 {
                return Err("Vertex replay index outside literal vertex extent".into());
            }
            index_rows.push(index);
            uploads.push(serde_json::json!({"id":id,"position_bits":v.position.map(f32::to_bits),"normal_bits":v.normal.map(f32::to_bits),"color_bits":v.color.map(f32::to_bits),"uv_bits":v.uv.map(f32::to_bits)}));
        }
        let source = include_str!("../shaders/world.metal");
        let signature = "vertex Out world_vertex(";
        let start = source
            .find(signature)
            .ok_or("Actual world_vertex signature changed")?;
        let tail = source[start..]
            .find("\n}")
            .ok_or("Actual world_vertex body extent changed")?
            + start
            + 2;
        let actual = &source[start..tail];
        let body_start = actual
            .find(" Out o;")
            .ok_or("Actual vertex arithmetic start changed")?;
        let remap =
            " o.position.z=(o.position.z+o.position.w)*0.5; o.color=float4(v[id].color); return o;";
        if actual.matches(remap).count() != 1 {
            return Err("Actual vertex arithmetic tail changed".into());
        }
        let replay=actual[..body_start].replace("vertex Out world_vertex(","vertex void observe_vertex(").replace("constant Section& section [[buffer(3)]])","constant Section& section [[buffer(3)]],device ReplayRecord* records [[buffer(4)]])")
   +&actual[body_start..].replace(remap," float4 gl_clip=o.position; o.position.z=(o.position.z+o.position.w)*0.5; records[id]={world,gl_clip,o.position,float4(o.clip,0.,0.,0.)};");
        let shader=source[..start].to_owned()+"\nstruct ReplayRecord {float4 world;float4 gl_clip;float4 metal_clip;float4 distance;};\n"+&replay;
        let library = metal
            .device
            .newLibraryWithSource_options_error(&NSString::from_str(&shader), None)
            .map_err(|e| format!("Vertex replay shader:{e:?}"))?;
        let function = library
            .newFunctionWithName(&NSString::from_str("observe_vertex"))
            .ok_or("Vertex replay function absent")?;
        let descriptor = MTLRenderPipelineDescriptor::new();
        descriptor.setVertexFunction(Some(&function));
        descriptor.setRasterizationEnabled(false);
        descriptor.setRasterSampleCount(1);
        let pipeline = metal
            .device
            .newRenderPipelineStateWithDescriptor_error(&descriptor)
            .map_err(|e| format!("Vertex replay pipeline:{e:?}"))?;
        let output = metal.buffer(60 * std::mem::size_of::<Record>(), true)?;
        unsafe {
            std::ptr::write_bytes(
                output.raw.contents().as_ptr().cast::<u8>(),
                0xff,
                output.length(),
            );
        }
        let command = metal
            .queue
            .commandBuffer()
            .ok_or("Vertex replay command absent")?;
        let pass = MTLRenderPassDescriptor::new();
        pass.setRenderTargetWidth(1);
        pass.setRenderTargetHeight(1);
        let encoder = command
            .renderCommandEncoderWithDescriptor(&pass)
            .ok_or("Vertex replay encoder absent")?;
        encoder.setRenderPipelineState(&pipeline);
        unsafe {
            encoder.setVertexBuffer_offset_atIndex(Some(&vertices.raw), 0, 0);
            encoder.setVertexBuffer_offset_atIndex(Some(&models.raw), 0, 1);
            encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(&self.mvp).cast(), 64, 2);
            encoder.setVertexBytes_length_atIndex(
                std::ptr::NonNull::from(&self.section).cast(),
                std::mem::size_of_val(&self.section),
                3,
            );
            encoder.setVertexBuffer_offset_atIndex(Some(&output.raw), 0, 4);
            encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Point, 0, 60);
        }
        encoder.endEncoding();
        command.commit();
        let flight = Flight {
            poison: metal.poisoned.clone(),
            command: ManuallyDrop::new(command),
            frame: ManuallyDrop::new(self.clone()),
            output: ManuallyDrop::new(output),
            pipeline: ManuallyDrop::new(pipeline),
        };
        let begin = std::time::Instant::now();
        while flight.command.status() != MTLCommandBufferStatus::Completed {
            if flight.command.status() == MTLCommandBufferStatus::Error {
                return Err(format!(
                    "Vertex replay command error:{:?}",
                    flight.command.error()
                ));
            }
            if begin.elapsed() > std::time::Duration::from_secs(5) {
                metal
                    .poisoned
                    .store(true, std::sync::atomic::Ordering::Release);
                return Err("Vertex replay completion deadline; owner quarantined".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let mut rows = Vec::with_capacity(60);
        for id in 0..60 {
            let v: Record = read(&flight.output, id)?;
            if v.world
                .iter()
                .chain(&v.gl_clip)
                .chain(&v.metal_clip)
                .chain(&v.distance)
                .any(|x| !x.is_finite())
            {
                return Err("Vertex replay unwritten/nonfinite output".into());
            }
            rows.push(serde_json::json!({"id":id,"world_bits":v.world.map(f32::to_bits),"gl_clip_bits":v.gl_clip.map(f32::to_bits),"metal_clip_bits":v.metal_clip.map(f32::to_bits),"clip_distance_bits":v.distance[0].to_bits()}));
        }
        let model: [[f32; 4]; 4] = read(models, 0)?;
        Ok(
            serde_json::json!({"separately_compiled_vertex_replay":true,"production_vs_instruction_identity_claim":false,"compiler_options":"None (same as production)","completed":true,"output_shared":flight.output.raw.storageMode()==MTLStorageMode::Shared,"output_bytes":flight.output.length(),"vertex_stride":std::mem::size_of::<ingetrazo_render::buffer::Vertex>(),"indices":index_rows,"uploads":uploads,"camera_bits":self.mvp.map(|c|c.map(f32::to_bits)),"model_bits":model.map(|c|c.map(f32::to_bits)),"section_plane_bits":self.section.plane_for_probe().map(f32::to_bits),"domain":domain,"outputs":rows,"actual_vertex_function_source":actual,"replay_shader_source":shader}),
        )
    }
}
