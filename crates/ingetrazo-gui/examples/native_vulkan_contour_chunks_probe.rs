//! Actual GPU normal/chunked equality, warm cache and live-quota rollback controls.
use ash::vk;
use glam::DVec3;
use ingetrazo_core::{
    camera::{OrbitCamera, StandardView},
    scene::Scene,
    section::SectionPlane,
};
use ingetrazo_gui::native_vulkan::{
    budget::VulkanBudget, headless::DeviceRequest, world::World, VulkanContext,
};
use std::{sync::Arc, time::Instant};
fn scene() -> Scene {
    let mut s = Scene::new();
    let outer = [[0., 0.], [1., 0.], [1., 1.], [0., 1.]];
    let inner = [[0.3, 0.3], [0.7, 0.3], [0.7, 0.7], [0.3, 0.7]];
    let rings = [0., 2.].map(|z| {
        [outer, inner].map(|ring| ring.map(|p| s.mesh.add_vertex(DVec3::new(p[0], p[1], z))))
    });
    s.mesh
        .add_face(
            rings[0][0].into_iter().rev().collect(),
            vec![rings[0][1].to_vec()],
        )
        .unwrap();
    s.mesh
        .add_face(
            rings[1][0].to_vec(),
            vec![rings[1][1].into_iter().rev().collect()],
        )
        .unwrap();
    for i in 0..4 {
        let j = (i + 1) % 4;
        s.mesh
            .add_face(
                vec![
                    rings[0][0][i],
                    rings[0][0][j],
                    rings[1][0][j],
                    rings[1][0][i],
                ],
                vec![],
            )
            .unwrap();
        s.mesh
            .add_face(
                vec![
                    rings[0][1][j],
                    rings[0][1][i],
                    rings[1][1][i],
                    rings[1][1][j],
                ],
                vec![],
            )
            .unwrap();
    }
    s.section_planes.push(SectionPlane::new(
        DVec3::new(0., 0., 1.5),
        DVec3::Z,
        "A",
        "A",
    ));
    s.show_section_cuts = true;
    s.show_section_fill = true;
    s
}
struct GpuTimer {
    device: ash::Device,
    pool: vk::QueryPool,
    bits: u32,
    period: f32,
    last_ms: std::cell::Cell<Option<f64>>,
}
impl GpuTimer {
    fn new(context: &VulkanContext) -> Result<Option<Self>, vk::Result> {
        if let Some((bits, period)) = context.graphics_timestamp_info() {
            let pool = unsafe {
                context.device.create_query_pool(
                    &vk::QueryPoolCreateInfo::default()
                        .query_type(vk::QueryType::TIMESTAMP)
                        .query_count(2),
                    None,
                )
            }?;
            Ok(Some(Self {
                device: context.device.clone(),
                pool,
                bits,
                period,
                last_ms: std::cell::Cell::new(None),
            }))
        } else {
            Ok(None)
        }
    }
}
impl Drop for GpuTimer {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_query_pool(self.pool, None);
        }
    }
}
fn render(
    context: &VulkanContext,
    timer: Option<&GpuTimer>,
    world: &mut World,
    fence: vk::Fence,
    command: vk::CommandBuffer,
    serial: u64,
) -> Result<Vec<u8>, String> {
    unsafe {
        context
            .device
            .reset_command_buffer(command, vk::CommandBufferResetFlags::empty())
            .map_err(|e| e.to_string())?;
        context
            .device
            .begin_command_buffer(command, &vk::CommandBufferBeginInfo::default())
            .map_err(|e| e.to_string())?;
    }
    if let Some(timer) = timer {
        unsafe {
            context
                .device
                .cmd_reset_query_pool(command, timer.pool, 0, 2);
            context.device.cmd_write_timestamp(
                command,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                timer.pool,
                0,
            );
        }
    }
    world.record(command, serial)?;
    if let Some(timer) = timer {
        unsafe {
            context.device.cmd_write_timestamp(
                command,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                timer.pool,
                1,
            );
        }
    }
    unsafe {
        context
            .device
            .end_command_buffer(command)
            .map_err(|e| e.to_string())?;
        context
            .device
            .reset_fences(&[fence])
            .map_err(|e| e.to_string())?;
        context
            .device
            .queue_submit(
                context.graphics_queue,
                &[vk::SubmitInfo::default().command_buffers(&[command])],
                fence,
            )
            .map_err(|e| e.to_string())?;
    }
    context.wait_transfer(fence).map_err(|e| e.to_string())?;
    if let Some(timer) = timer {
        let mut ticks = [0u64; 2];
        unsafe {
            context.device.get_query_pool_results(
                timer.pool,
                0,
                &mut ticks,
                vk::QueryResultFlags::TYPE_64,
            )
        }
        .map_err(|e| e.to_string())?;
        let mask = if timer.bits == 64 {
            u64::MAX
        } else {
            (1u64 << timer.bits) - 1
        };
        let elapsed = ticks[1].wrapping_sub(ticks[0]) & mask;
        timer
            .last_ms
            .set(Some(elapsed as f64 * timer.period as f64 / 1e6));
        eprintln!(
            "Actual full World GPUdraw timestamp serial{serial}: {:.6}ms",
            elapsed as f64 * timer.period as f64 / 1e6
        );
    }
    world.readback(context)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let context =
        VulkanContext::new_headless(&if args.iter().any(|a| a == "--software-fixture") {
            DeviceRequest::software_fixture()
        } else {
            DeviceRequest::nvidia_hardware()
        })?;
    let budget = Arc::new(VulkanBudget::new(
        &context.instance,
        context.physical_device,
    )?);
    budget.ledger.lock().unwrap().total_limit = Some(128 * 1024 * 1024);
    let fence = unsafe {
        context.device.create_fence(
            &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
            None,
        )
    }?;
    let commands = unsafe {
        context.device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default()
                .command_pool(context.command_pool)
                .command_buffer_count(1),
        )
    }?;
    let command = commands[0];
    let timer = GpuTimer::new(&context)?;
    if let Some(count) = args
        .windows(2)
        .find(|a| a[0] == "--perf-triangles")
        .map(|a| a[1].parse::<usize>())
        .transpose()?
    {
        if count == 0 || count > 250000 {
            return Err("Bounded performance fixture accepts 1..=250000 triangles".into());
        }
        let transparent = args.iter().any(|a| a == "--transparent");
        let limit = args
            .windows(2)
            .find(|a| a[0] == "--storage-limit")
            .map(|a| a[1].parse::<u64>())
            .transpose()?;
        // Linear fixture generation; bypass edge topology since this rendering probe owns no edits.
        let mut scene = Scene::new();
        let columns = (count as f64).sqrt().ceil() as usize;
        let step = 1. / columns as f64;
        for index in 0..count {
            let x = (index % columns) as f64 * step;
            let y = (index / columns) as f64 * step;
            let first = scene.mesh.vertices.len();
            for p in [
                DVec3::new(x, y, 1.),
                DVec3::new(x + step * 0.9, y, 2.),
                DVec3::new(x, y + step * 0.9, 2.),
            ] {
                scene
                    .mesh
                    .vertices
                    .push(ingetrazo_core::mesh::MeshVertex::new(p));
            }
            let mut face = ingetrazo_core::mesh::MeshFace::new(vec![first, first + 1, first + 2]);
            if transparent {
                face.attrs.insert("opacity".into(), serde_json::json!(0.5));
            }
            scene.mesh.faces.push(face);
        }
        scene.section_planes.push(SectionPlane::new(
            DVec3::new(0., 0., 1.5),
            DVec3::Z,
            "A",
            "A",
        ));
        scene.show_section_cuts = true;
        scene.show_section_fill = true;
        let mut camera = OrbitCamera::default();
        camera.set_standard_view(StandardView::Top);
        camera.target = DVec3::new(0.5, 0.5, 1.5);
        camera.distance = 5.;
        camera.perspective = false;
        camera.aspect = 800. / 600.;
        let style = ingetrazo_render::style::StyleConfig::default();
        let mut world = World::new(&context, budget.clone())?;
        if let Some(limit) = limit {
            world.set_contour_storage_limit_for_probe(limit)?;
        }
        world.resize_offscreen(
            &context,
            vk::Extent2D {
                width: 800,
                height: 600,
            },
        )?;
        world.set_scene(&context, &scene, &camera, &style, fence, 0, 1)?;
        let cold = world.contour_metrics();
        let charged = budget.ledger.lock().unwrap().total_committed();
        let mut times = Vec::new();
        for serial in 1..=11 {
            world.set_scene(&context, &scene, &camera, &style, fence, serial - 1, 1)?;
            let image = render(&context, timer.as_ref(), &mut world, fence, command, serial)?;
            assert_eq!(image.len(), 800 * 600 * 4);
            assert_eq!(world.uploads, 1);
            assert_eq!(
                budget.ledger.lock().unwrap().total_committed(),
                charged,
                "Warm frame allocated additional GPU memory"
            );
            if serial > 1 {
                times.push(timer.as_ref().and_then(|t| t.last_ms.get()));
            }
        }
        let peak = budget.ledger.lock().unwrap().peak_committed;
        drop(world);
        assert_eq!(budget.ledger.lock().unwrap().total_committed(), 0);
        let result = serde_json::json!({"triangles":count,"transparent":transparent,"forced_storage_limit":limit,"additional_position_bytes":cold.additional_position_bytes,"contour_batches":cold.batches,"cold_build_ms":cold.cold_build_ms,"compact_preprocessing_ms":cold.compact_preprocess_ms,"gpu_draw_ms_after_one_warmup":times,"actual_charged_bytes":charged,"peak_charged_bytes":peak,"warm_geometry_uploads":0,"warm_gpu_allocations":0,"final_ledger_bytes":0,"scope":"Bounded direct GPU world+contour command timestamps, 800x600/4x MSAA. Diagnostic GPU readback occurs after each timestamp interval; shader first use excluded by one warmup. Synthetic linear corpus, not representative application throughput."});
        if let Some(output) = args
            .windows(2)
            .find(|a| a[0] == "--output")
            .map(|a| std::path::Path::new(&a[1]))
        {
            std::fs::write(output, serde_json::to_vec_pretty(&result)?)?;
        }
        println!("{}", serde_json::to_string_pretty(&result)?);
        unsafe {
            context
                .device
                .free_command_buffers(context.command_pool, &commands);
            context.device.destroy_fence(fence, None);
        }
        return Ok(());
    }
    if let Some(root) = args
        .windows(2)
        .find(|a| a[0] == "--corpus-root")
        .map(|a| std::path::Path::new(&a[1]))
    {
        let out = args
            .windows(2)
            .find(|a| a[0] == "--output")
            .map(|a| std::path::PathBuf::from(&a[1]))
            .unwrap_or_else(|| "/tmp/contour-chunk-corpus".into());
        std::fs::create_dir_all(&out)?;
        let expected = [
            ("original", vec!["closed-front", "closed-iso", "closed", "coplanar-backface", "duplicate", "shared-slab-iso", "shared-slab-reversed-iso", "shared-slab-reversed", "shared-slab", "transparent"]),
            ("hollow-original", vec!["hollow-iso", "hollow-seam", "hollow", "transparent-hollow"]),
        ];
        for (directory, names) in expected {
            for name in names {
                if !root.join(directory).join(format!("{name}.igz")).is_file() {
                    return Err(format!("Missing mandatory original corpus case {directory}/{name}.igz").into());
                }
            }
        }
        let mut serial = 0;
        for directory in ["original", "hollow-original"] {
            let mut inputs = std::fs::read_dir(root.join(directory))?
                .map(|p| p.unwrap().path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("igz"))
                .collect::<Vec<_>>();
            inputs.sort();
            for input in inputs {
                let scene = ingetrazo_formats::igz::load_igz(&input)?;
                let name = input.file_stem().unwrap().to_str().unwrap();
                let mut camera = OrbitCamera::default();
                camera.set_standard_view(if name.ends_with("iso") {
                    StandardView::Iso
                } else if name.ends_with("front") {
                    StandardView::Front
                } else {
                    StandardView::Top
                });
                camera.target = DVec3::new(0.5, 0.5, 1.5);
                camera.distance = 5.;
                camera.perspective = false;
                camera.aspect = 800. / 600.;
                let mut style = ingetrazo_render::style::StyleConfig::default();
                style.section_fill_color = [1., 0., 1., 1.];
                let mut images = Vec::new();
                for limit in [None, Some(36)] {
                    let mut world = World::new(&context, budget.clone())?;
                    if let Some(limit) = limit {
                        world.set_contour_storage_limit_for_probe(limit)?;
                    }
                    world.resize_offscreen(
                        &context,
                        vk::Extent2D {
                            width: 800,
                            height: 600,
                        },
                    )?;
                    world.set_scene(&context, &scene, &camera, &style, fence, serial, 1)?;
                    serial += 1;
                    images.push(render(
                        &context,
                        timer.as_ref(),
                        &mut world,
                        fence,
                        command,
                        serial,
                    )?);
                    eprintln!(
                        "Corpus {name} limit{limit:?}: {:?}",
                        world.contour_metrics()
                    );
                    drop(world);
                    assert_eq!(budget.ledger.lock().unwrap().total_committed(), 0);
                }
                assert_eq!(
                    images[0], images[1],
                    "Original source {name} changed pixels across GPU storage chunks"
                );
                image::save_buffer(
                    out.join(format!("{name}.png")),
                    &images[1],
                    800,
                    600,
                    image::ColorType::Rgba8,
                )?;
                println!(
                    "PASS source IGZ {name}: normal/36-byte GPU chunks exact whole-image equality"
                );
            }
        }
        unsafe {
            context
                .device
                .free_command_buffers(context.command_pool, &commands);
            context.device.destroy_fence(fence, None);
        }
        return Ok(());
    }
    let mut s = scene();
    let mut camera = OrbitCamera::default();
    camera.set_standard_view(StandardView::Top);
    camera.target = DVec3::new(0.5, 0.5, 1.5);
    camera.distance = 5.;
    camera.perspective = false;
    camera.aspect = 800. / 600.;
    let mut style = ingetrazo_render::style::StyleConfig::default();
    style.section_fill_color = [1., 0., 1., 1.];
    style.edge_color = [0., 0., 0., 1.];
    let mut serial = 0;
    let mut baseline = World::new(&context, budget.clone())?;
    baseline.resize_offscreen(
        &context,
        vk::Extent2D {
            width: 800,
            height: 600,
        },
    )?;
    baseline.set_scene(&context, &s, &camera, &style, fence, serial, 1)?;
    serial += 1;
    let normal = render(
        &context,
        timer.as_ref(),
        &mut baseline,
        fence,
        command,
        serial,
    )?;
    assert_eq!(baseline.contour_metrics().additional_position_bytes, 0);
    drop(baseline);
    let mut world = World::new(&context, budget.clone())?;
    world.set_contour_storage_limit_for_probe(72)?;
    world.resize_offscreen(
        &context,
        vk::Extent2D {
            width: 800,
            height: 600,
        },
    )?;
    world.set_scene(&context, &s, &camera, &style, fence, serial, 1)?;
    serial += 1;
    let chunked = render(&context, timer.as_ref(), &mut world, fence, command, serial)?;
    assert_eq!(
        normal, chunked,
        "Forced chunks changed hollow section pixels"
    );
    assert!(world.contour_batch_count() > 2);
    assert!(world.contour_metrics().additional_position_bytes > 0);
    let baseline_uploads = world.uploads;
    let cold = world.contour_metrics();
    let started = Instant::now();
    for _ in 0..100 {
        world.set_scene(&context, &s, &camera, &style, fence, serial, 1)?;
    }
    let warm_average_ms = started.elapsed().as_secs_f64() * 10.;
    assert_eq!(world.uploads, baseline_uploads);
    // Camera, edge color and plane uniforms change the rendered result without uploading world geometry.
    style.edge_color = [0., 1., 0., 1.];
    camera.distance = 4.;
    s.section_planes[0].point.z = 1.25;
    world.set_scene(&context, &s, &camera, &style, fence, serial, 1)?;
    assert_eq!(world.uploads, baseline_uploads);
    serial += 1;
    let uniforms = render(&context, timer.as_ref(), &mut world, fence, command, serial)?;
    assert_ne!(uniforms, normal);
    style.edge_color = [0., 0., 0., 1.];
    camera.distance = 5.;
    s.section_planes[0].point.z = 1.5;
    world.set_scene(&context, &s, &camera, &style, fence, serial, 1)?;
    serial += 1;
    let stable = render(&context, timer.as_ref(), &mut world, fence, command, serial)?;
    assert_eq!(stable, normal);
    // Actual quota denials after partially preparing chunk resources must preserve the old drawable.
    let before = budget.ledger.lock().unwrap().total_committed();
    let old_edge_cache=(world.edge_endpoint_count(),world.edge_geometry_bytes());
    let mut changed = s.clone();
    for face in &mut changed.mesh.faces {
        face.attrs
            .insert("color".into(), serde_json::json!([0., 0., 1., 1.]));
    }
    changed.section_planes[0].normal = DVec3::X;
    changed.section_planes[0].point = DVec3::new(0.5, 0., 0.);
    changed.bump_version();
    // Lower live admission only after actual chunk allocations, rather than assuming
    // driver staging requirements allow a fixed-cap partial failure window.
    world.set_contour_pressure_after_chunks_for_probe(Some(2))?;
    let old_style=world.style_state();let mut rejected_style=style.clone();rejected_style.visual_style=ingetrazo_render::style::VisualStyle::Monochrome;rejected_style.front_color=[0.,1.,0.,1.];rejected_style.back_color=[1.,0.,0.,1.];
    let result = world.set_scene(&context, &changed, &camera, &rejected_style, fence, serial, 2);
    assert!(
        result.is_err(),
        "Dynamic quota pressure unexpectedly admitted the next chunk"
    );
    assert!(result.unwrap_err().contains("Contour chunk upload failed"));
    assert_eq!((world.edge_endpoint_count(),world.edge_geometry_bytes()),old_edge_cache,"Rejected replacement changed cached drawable edges");
    assert_eq!(world.contour_metrics().failed_prepared_chunks, 2);
    assert_eq!(
        budget.ledger.lock().unwrap().total_committed(),
        before,
        "Actual partial chunk rollback leaked allocations"
    );
    assert_eq!(
        budget.ledger.lock().unwrap().total_limit,
        Some(128 * 1024 * 1024),
        "Engineering pressure did not restore the original budget"
    );
    assert_eq!(world.uploads, baseline_uploads);
    assert_eq!(world.style_state(),old_style,"Failed chunk candidate changed cached style state");
    world.set_contour_pressure_after_chunks_for_probe(None)?;
    serial += 1;
    let restored = render(&context, timer.as_ref(), &mut world, fence, command, serial)?;
    assert_eq!(
        restored, stable,
        "Failed chunk preparation changed old GPU drawable"
    );
    // Opaque -> transparent moves geometry out of stencil but retains all contour chunks.
    for face in &mut s.mesh.faces {
        face.attrs.insert("opacity".into(), serde_json::json!(0.5));
    }
    s.bump_version();
    world.set_scene(&context, &s, &camera, &style, fence, serial, 3)?;
    serial += 1;
    let transparent = render(&context, timer.as_ref(), &mut world, fence, command, serial)?;
    assert_ne!(transparent, normal);
    assert!(world.contour_metrics().additional_position_bytes > 0);
    assert_eq!(world.uploads, baseline_uploads + 1);
    let output = args
        .windows(2)
        .find(|a| a[0] == "--output")
        .map(|a| std::path::PathBuf::from(&a[1]))
        .unwrap_or_else(|| "/tmp/contour-chunks.png".into());
    image::save_buffer(&output, &normal, 800, 600, image::ColorType::Rgba8)?;
    println!("PASS actual GPU hollow normal/chunked exact image equality; cold {:?}; warm100average{warm_average_ms:.6}ms; uniform cache, actualpartialquota rollback oldimage, opacity invalidation; final {:?}",cold,world.contour_metrics());
    drop(world);
    assert_eq!(budget.ledger.lock().unwrap().total_committed(), 0);
    unsafe {
        context
            .device
            .free_command_buffers(context.command_pool, &commands);
        context.device.destroy_fence(fence, None);
    }
    Ok(())
}
