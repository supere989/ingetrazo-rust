//! Focused real GPU acceptance; unsupported platforms cannot pretend to pass.
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Native Metal world acceptance requires macOS");
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
        use ingetrazo_metal::{world::World, NativeMetal};
        use ingetrazo_render::buffer::{MeshRenderBuffer, Vertex};
        let metal = NativeMetal::new()?;
        let mut world = World::default();
        let identity = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ];
        let packet = |near_first: bool| {
            let mut mesh = MeshRenderBuffer::default();
            for (z, color) in if near_first {
                [(-0.5, [0., 1., 0., 1.]), (0.5, [1., 0., 0., 1.])]
            } else {
                [(0.5, [1., 0., 0., 1.]), (-0.5, [0., 1., 0., 1.])]
            } {
                for p in [[-0.8, -0.8, z], [0.8, -0.8, z], [0., 0.8, z]] {
                    mesh.vertices.push(Vertex {
                        position: p,
                        normal: [0., 0., 1.],
                        color,
                        uv: [0.; 2],
                    });
                }
            }
            mesh.indices = (0..6).collect();
            mesh
        };
        for order in 0..2 {
            let frame = world
                .prepare(&metal, order, identity, (64, 64), [0., 0., 1., 1.], || {
                    Ok(packet(order == 1))
                })?
                .ok_or("Target unavailable")?;
            let pixels = frame
                .submit_diagnostic(&metal)?
                .readback(std::time::Duration::from_secs(5))?;
            assert_eq!(
                &pixels[(32 * 64 + 32) * 4..(32 * 64 + 32) * 4 + 4],
                &[0, 255, 0, 255],
                "near green must occlude red in either draw order"
            );
            assert_eq!(&pixels[..4], &[0, 0, 255, 255], "clear outside triangle");
        }
        let frame = world
            .prepare(&metal, 1, identity, (64, 64), [0., 0., 1., 1.], || {
                panic!("negative must reuse admitted geometry")
            })?
            .ok_or("Negative target unavailable")?;
        let negative = frame
            .submit_diagnostic_without_depth(&metal)?
            .readback(std::time::Duration::from_secs(5))?;
        assert_eq!(
            &negative[(32 * 64 + 32) * 4..(32 * 64 + 32) * 4 + 4],
            &[255, 0, 0, 255],
            "literal no-depth mutation must expose later far red"
        );
        drop(frame);
        let uploads = world.stats.geometry_uploads;
        let targets = world.stats.target_allocations;
        for _ in 0..3 {
            let f = world
                .prepare(&metal, 1, identity, (64, 64), [0., 0., 1., 1.], || {
                    panic!("warm geometry collector invoked")
                })?
                .ok_or("Warm target unavailable")?;
            f.submit_diagnostic(&metal)?
                .readback(std::time::Duration::from_secs(5))?;
        }
        assert_eq!(world.stats.geometry_uploads, uploads);
        assert_eq!(world.stats.target_allocations, targets);
        let mut invalid = identity;
        invalid[0][0] = f32::NAN;
        assert!(world
            .prepare(&metal, 1, invalid, (64, 64), [0., 0., 1., 1.], || panic!(
                "bad pose collected"
            ))
            .is_err());
        println!("Native Metal world opaque/depth: two draw orders, clear, three warm frames and invalid pose PASS; device={}; uploads={uploads}; targets={targets}",metal.info.name);
        Ok(())
    })
}
