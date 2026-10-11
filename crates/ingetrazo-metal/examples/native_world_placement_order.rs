//! Source-global prototype bucket order through actual native Metal draws.
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Native Metal placement order requires macOS");
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
            world_policy::Policy,
            NativeMetal,
        };
        use ingetrazo_render::buffer::{MeshRenderBuffer, Vertex};
        let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("Output required")?);
        std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        let metal = NativeMetal::new()?;
        let mut world = World::default();
        let key = |identity| PrototypeKey {
            identity,
            geometry_revision: 1,
            paint_revision: 1,
            style_revision: 1,
            layer_revision: 1,
        };
        let a = key(1);
        let b = key(2);
        let c = key(3);
        let identity = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ];
        let poses = [Pose {
            mvp: identity,
            reflected: false,
        }; 3];
        let collect = |k: &PrototypeKey| {
            let color = if k.identity == 2 {
                [0.96875, 0., 0., 1.]
            } else {
                [0., 0.96875, 0., 1.]
            };
            let mut m = MeshRenderBuffer::default();
            for p in [[-0.6, -0.6, 0.], [0.6, -0.6, 0.], [0., 0.6, 0.]] {
                m.vertices.push(Vertex {
                    position: p,
                    normal: [0., 0., 1.],
                    color,
                    uv: [0.; 2],
                });
            }
            m.indices = vec![0, 1, 2];
            Ok(m)
        };
        let packet = Packet::new(vec![a, b], vec![0, 1, 0])?;
        let first = world
            .prepare_placements_with_camera(
                &metal,
                packet.clone(),
                &poses,
                identity,
                (64, 64),
                [0., 0., 1., 1.],
                Policy::default(),
                collect,
            )?
            .ok_or("Grouped frame unavailable")?;
        assert_eq!(first.instance_run_count(), 2);
        let read = |f: std::sync::Arc<ingetrazo_metal::world::Prepared>| {
            f.submit_diagnostic(&metal)?
                .readback(std::time::Duration::from_secs(5))
        };
        std::fs::write(out.join("source-global-order.rgba"), read(first)?)
            .map_err(|e| e.to_string())?;
        let before = world.stats;
        let mut camera = identity;
        camera[3][0] = 0.05;
        let warm = world
            .prepare_placements_with_camera(
                &metal,
                packet,
                &poses,
                camera,
                (64, 64),
                [0., 0., 1., 1.],
                Policy::default(),
                |_| panic!("Warm grouped prototype collected"),
            )?
            .ok_or("Warm frame unavailable")?;
        std::fs::write(out.join("camera-only-global-order.rgba"), read(warm)?)
            .map_err(|e| e.to_string())?;
        let after = world.stats;
        assert_eq!(before.geometry_uploads, after.geometry_uploads);
        assert_eq!(before.draw_order_builds, after.draw_order_builds);
        assert_eq!(
            before.instance_matrix_upload_bytes,
            after.instance_matrix_upload_bytes
        );
        let negative = world
            .prepare_placements_with_camera(
                &metal,
                Packet::new(vec![a, b, c], vec![0, 1, 2])?,
                &poses,
                identity,
                (64, 64),
                [0., 0., 1., 1.],
                Policy::default(),
                collect,
            )?
            .ok_or("Independent identity negative unavailable")?;
        assert_eq!(negative.instance_run_count(), 3);
        std::fs::write(
            out.join("independent-last-prototype-negative.rgba"),
            read(negative)?,
        )
        .map_err(|e| e.to_string())?;
        drop(world);
        let budget = metal.budget.lock().map_err(|_| "Budget poisoned")?;
        let committed: u64 = budget.heaps.iter().map(|h| h.committed).sum();
        let reserved: u64 = budget.heaps.iter().map(|h| h.reserved).sum();
        assert_eq!((committed, reserved), (0, 0));
        std::fs::write(out.join("report.json"),format!("{{\"passed\":true,\"grouped_runs\":2,\"independent_runs\":3,\"geometry_uploads_before_warm\":{},\"geometry_uploads_after_warm\":{},\"order_builds_before_warm\":{},\"order_builds_after_warm\":{},\"matrix_bytes_before_warm\":{},\"matrix_bytes_after_warm\":{},\"ledger_committed\":{committed},\"ledger_reserved\":{reserved}}}\n",before.geometry_uploads,after.geometry_uploads,before.draw_order_builds,after.draw_order_builds,before.instance_matrix_upload_bytes,after.instance_matrix_upload_bytes)).map_err(|e|e.to_string())?;
        Ok(())
    })
}
