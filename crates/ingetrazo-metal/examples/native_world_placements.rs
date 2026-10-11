//! Actual direct native placement consumer; no UI/presentation acceptance claim.
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Native Metal placement fixture requires macOS");
    std::process::exit(2);
}
#[cfg(target_os = "macos")]
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
#[cfg(target_os = "macos")]
fn run() -> Result<(), String> {
    objc2::rc::autoreleasepool(|_| {
        use ingetrazo_metal::{
            world::World,
            world_placement::{Packet, Pose, PrototypeKey},
            world_policy::{Mode, Policy},
            NativeMetal,
        };
        use ingetrazo_render::buffer::{MeshRenderBuffer, Vertex};
        let out = std::path::PathBuf::from(
            std::env::args()
                .nth(1)
                .ok_or("Placement output directory required")?,
        );
        std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        let metal = NativeMetal::new()?;
        let mut world = World::default();
        let key = PrototypeKey {
            identity: 1,
            geometry_revision: 1,
            paint_revision: 1,
            style_revision: 1,
            layer_revision: 1,
        };
        let packet = Packet::new(vec![key], vec![0, 0])?;
        let style = Policy {
            mode: Mode::Material,
            front: [1., 0., 0., 1.],
            back: [1., 1., 0., 1.],
        };
        let identity = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ];
        let pose = |x: f32, reflection: bool| {
            let mut m = identity;
            m[0][0] = if reflection { -1. } else { 1. };
            m[3][0] = x;
            Pose {
                mvp: m,
                reflected: reflection,
            }
        };
        let geometry = |color| {
            let mut m = MeshRenderBuffer::default();
            for p in [[-0.25, -0.25, 0.], [0.25, -0.25, 0.], [0., 0.25, 0.]] {
                m.vertices.push(Vertex {
                    position: p,
                    normal: [0., 0., 1.],
                    color,
                    uv: [0.; 2],
                });
            }
            m.indices = vec![0, 1, 2];
            m.source_default_back = vec![(0, 3)];
            m
        };
        let save = |name: &str, pixels: &[u8]| {
            std::fs::write(out.join(format!("{name}.rgba")), pixels).map_err(|e| e.to_string())
        };
        let read = |f: std::sync::Arc<ingetrazo_metal::world::Prepared>| {
            f.submit_diagnostic(&metal)?
                .readback(std::time::Duration::from_secs(5))
        };
        let first = world
            .prepare_placements(
                &metal,
                packet.clone(),
                &[pose(-0.4, false), pose(0.4, true)],
                (64, 64),
                [0., 0., 1., 1.],
                style,
                |_| Ok(geometry([0., 1., 0., 1.])),
            )?
            .ok_or("First placement target unavailable")?;
        save("shared-prototype-reflected", &read(first)?)?;
        assert_eq!(world.stats.geometry_uploads, 1);
        assert_eq!(world.stats.target_allocations, 1);
        let coalesced = world
            .prepare_placements(
                &metal,
                packet.clone(),
                &[pose(-0.4, false), pose(0.4, false)],
                (64, 64),
                [0., 0., 1., 1.],
                style,
                |_| panic!("Coalesced prototype collected"),
            )?
            .ok_or("Coalesced target unavailable")?;
        assert_eq!(coalesced.instance_run_count(), 1);
        save("coalesced-two-instances", &read(coalesced)?)?;
        let before = world.stats;
        for (name, x) in [
            ("warm-left", -0.5),
            ("warm-center", -0.4),
            ("warm-right", -0.3),
        ] {
            let f = world
                .prepare_placements(
                    &metal,
                    packet.clone(),
                    &[pose(x, false), pose(0.4, true)],
                    (64, 64),
                    [0., 0., 1., 1.],
                    style,
                    |_| panic!("Warm prototype collected"),
                )?
                .ok_or("Warm placement target unavailable")?;
            save(name, &read(f)?)?;
        }
        let after = world.stats;
        let unchanged = world
            .prepare_placements(
                &metal,
                packet.clone(),
                &[pose(-0.3, false), pose(0.4, true)],
                (64, 64),
                [0., 0., 1., 1.],
                style,
                |_| panic!("Unchanged prototype collected"),
            )?
            .ok_or("Unchanged frame unavailable")?;
        assert_eq!(
            world.stats.instance_matrix_upload_bytes,
            after.instance_matrix_upload_bytes
        );
        drop(unchanged);
        let mut camera = identity;
        camera[3][0] = -0.1;
        let camera_only = world
            .prepare_placements_with_camera(
                &metal,
                packet.clone(),
                &[pose(-0.3, false), pose(0.4, true)],
                camera,
                (64, 64),
                [0., 0., 1., 1.],
                style,
                |_| panic!("Camera-only prototype collected"),
            )?
            .ok_or("Camera frame unavailable")?;
        assert_eq!(
            world.stats.instance_matrix_upload_bytes,
            after.instance_matrix_upload_bytes
        );
        save("camera-only-no-instance-upload", &read(camera_only)?)?;
        let offset_packet = Packet::new(vec![key], vec![0, 0, 0, 0])?;
        let offset = world
            .prepare_placements(
                &metal,
                offset_packet,
                &[
                    pose(-0.6, false),
                    pose(-0.2, false),
                    pose(0.2, true),
                    pose(0.6, true),
                ],
                (64, 64),
                [0., 0., 1., 1.],
                style,
                |_| panic!("Offset run collected"),
            )?
            .ok_or("Offset instance run unavailable")?;
        assert_eq!(offset.instance_run_count(), 2);
        save("nonzero-offset-two-runs", &read(offset)?)?;

        assert_eq!(before.instance_buffer_allocations, 1);
        assert_eq!(
            before.instance_buffer_allocations,
            after.instance_buffer_allocations
        );
        assert_eq!(
            after.instance_matrix_upload_bytes - before.instance_matrix_upload_bytes,
            3 * 128
        );
        assert_eq!(before.geometry_uploads, after.geometry_uploads);
        assert_eq!(before.target_allocations, after.target_allocations);
        assert_eq!(
            before.pose_storage_payload_bytes,
            after.pose_storage_payload_bytes
        );
        assert_eq!(
            before.placement_cache_payload_bytes,
            after.placement_cache_payload_bytes
        );
        let wrong_winding = world
            .prepare_placements(
                &metal,
                packet.clone(),
                &[pose(-0.4, false), {
                    let mut p = pose(0.4, true);
                    p.reflected = false;
                    p
                }],
                (64, 64),
                [0., 0., 1., 1.],
                style,
                |_| panic!("Winding negative collected"),
            )?
            .ok_or("Winding negative target unavailable")?;
        save("winding-disabled-negative", &read(wrong_winding)?)?;
        let old = world
            .prepare_placements(
                &metal,
                packet.clone(),
                &[pose(-0.4, false), pose(0.4, true)],
                (64, 64),
                [0., 0., 1., 1.],
                style,
                |_| panic!("Old retained prototype collected"),
            )?
            .ok_or("Old target unavailable")?;
        let mut changed = key;
        changed.paint_revision += 1;
        let replacement_packet = Packet::new(vec![changed], vec![0, 0])?;
        let replacement = world
            .prepare_placements(
                &metal,
                replacement_packet.clone(),
                &[pose(-0.4, false), pose(0.4, true)],
                (64, 64),
                [0., 0., 1., 1.],
                style,
                |k| {
                    assert_eq!(*k, changed);
                    Ok(geometry([1., 0., 0., 1.]))
                },
            )?
            .ok_or("Replacement target unavailable")?;
        assert_eq!(world.stats.geometry_uploads, 2);
        save("old-retained-green", &read(old)?)?;
        save("new-revision-red", &read(replacement)?)?;
        // A new binding packet reuses unchanged prototype identities without collecting.
        let rebound = Packet::new(vec![changed], vec![0])?;
        let f = world
            .prepare_placements(
                &metal,
                rebound.clone(),
                &[pose(-0.4, false)],
                (64, 64),
                [0., 0., 1., 1.],
                style,
                |_| panic!("Binding-only change collected"),
            )?
            .ok_or("Binding target unavailable")?;
        save("binding-only-reuse", &read(f)?)?;
        assert_eq!(world.stats.geometry_uploads, 2);
        let before_refusal = world.stats;
        let mut invalid = pose(0., false);
        invalid.mvp[0][0] = f32::NAN;
        assert!(world
            .prepare_placements(
                &metal,
                rebound.clone(),
                &[invalid],
                (64, 64),
                [0., 0., 1., 1.],
                style,
                |_| panic!("Invalid pose collected")
            )
            .is_err());
        assert_eq!(world.stats.prepared, before_refusal.prepared);
        let foreign = NativeMetal::new()?;
        assert!(world
            .prepare_placements(
                &foreign,
                rebound.clone(),
                &[pose(-0.4, false)],
                (64, 64),
                [0., 0., 1., 1.],
                style,
                |_| panic!("Foreign owner collected")
            )
            .is_err());
        assert_eq!(world.stats.prepared, before_refusal.prepared);
        let f = world
            .prepare_placements(
                &metal,
                rebound,
                &[pose(-0.4, false)],
                (64, 64),
                [0., 0., 1., 1.],
                style,
                |_| panic!("Refusal lost prior prototype"),
            )?
            .ok_or("Preserved target unavailable")?;
        save("refusal-preserved", &read(f)?)?;
        drop(world);
        drop(foreign);
        let budget = metal.budget.lock().map_err(|_| "Budget poisoned")?;
        let committed: u64 = budget.heaps.iter().map(|h| h.committed).sum();
        let reserved: u64 = budget.heaps.iter().map(|h| h.reserved).sum();
        assert_eq!(committed, 0);
        assert_eq!(reserved, 0);
        drop(budget);
        let device = metal.info.name.replace('\\', "\\\\").replace('"', "\\\"");
        std::fs::write(out.join("report.json"),format!("{{\"passed\":true,\"device\":\"{device}\",\"geometry_uploads_before_warm\":{},\"geometry_uploads_after_warm\":{},\"targets_before_warm\":{},\"targets_after_warm\":{},\"pose_bytes_before_warm\":{},\"pose_bytes_after_warm\":{},\"cache_bytes_before_warm\":{},\"cache_bytes_after_warm\":{},\"instance_buffers_before_warm\":{},\"instance_buffers_after_warm\":{},\"warm_matrix_upload_bytes\":{},\"camera_only_instance_upload_bytes\":0,\"offset_runs\":2,\"coalesced_instances\":2,\"coalesced_runs\":1,\"final_uploads\":2,\"ledger_committed\":{committed},\"ledger_reserved\":{reserved},\"wrong_owner_refused\":true,\"invalid_pose_refused\":true}}\n",before.geometry_uploads,after.geometry_uploads,before.target_allocations,after.target_allocations,before.pose_storage_payload_bytes,after.pose_storage_payload_bytes,before.placement_cache_payload_bytes,after.placement_cache_payload_bytes,before.instance_buffer_allocations,after.instance_buffer_allocations,after.instance_matrix_upload_bytes-before.instance_matrix_upload_bytes)).map_err(|e|e.to_string())?;
        Ok(())
    })
}
