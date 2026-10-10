#[cfg(target_os = "macos")]
fn main() -> Result<(), String> {
    use ingetrazo_metal::{MetalCad2d, NativeMetal};
    use ingetrazo_render::cad2d::{Batch, EncodedRgba, Geometry, Pose, Vertex};
    use objc2_metal::MTLDevice;
    let metal = NativeMetal::new()?;
    println!(
        "Native Metal CAD device={} unified={} Apple1={} ROG={}; virtual/physical classification is external, never inferred from architecture",
        metal.info.name,
        metal.info.unified_memory,
        metal
            .device
            .supportsFamily(objc2_metal::MTLGPUFamily::Apple1),
        metal.device.areRasterOrderGroupsSupported()
    );
    if !MetalCad2d::supported(&metal) {
        assert!(MetalCad2d::new(metal, 64, 64).is_err());
        println!(
            "PASS explicit unsupported exact-framebuffer-fetch refusal; no rendered Metal image proof. Intel/AMD exact composition remains OPEN"
        );
        if std::env::args().any(|a| a == "--allow-unsupported") {
            return Ok(());
        }
        return Err("Exact Metal framebuffer fetch unsupported".into());
    }
    let budget = metal.budget.clone();
    let baseline = budget.lock().unwrap().total_committed();
    let mut renderer = MetalCad2d::new(metal, 64, 64)?;
    let mut vertices = Vec::new();
    for (a, b) in [(8., 56.), (16., 48.)] {
        for position in [[a, a], [b, a], [b, b], [a, b]] {
            vertices.push(Vertex {
                position,
                coverage: 1.,
            });
        }
    }
    let geometry = Geometry::new(vertices, vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7])?;
    let references: [(&str, u8, &[u8]); 6] = [
        (
            "alpha-0",
            0,
            include_bytes!("../../../docs/audits/metal-cad2d-offscreen/original/alpha-0.rgba"),
        ),
        (
            "alpha-1",
            1,
            include_bytes!("../../../docs/audits/metal-cad2d-offscreen/original/alpha-1.rgba"),
        ),
        (
            "alpha-127",
            127,
            include_bytes!("../../../docs/audits/metal-cad2d-offscreen/original/alpha-127.rgba"),
        ),
        (
            "alpha-128",
            128,
            include_bytes!("../../../docs/audits/metal-cad2d-offscreen/original/alpha-128.rgba"),
        ),
        (
            "alpha-254",
            254,
            include_bytes!("../../../docs/audits/metal-cad2d-offscreen/original/alpha-254.rgba"),
        ),
        (
            "alpha-255",
            255,
            include_bytes!("../../../docs/audits/metal-cad2d-offscreen/original/alpha-255.rgba"),
        ),
    ];
    let output =
        std::env::args().find_map(|a| a.strip_prefix("--output=").map(std::path::PathBuf::from));
    if let Some(ref p) = output {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    for (name, alpha, reference) in references {
        let batches = [
            Batch {
                first: 0,
                count: 6,
                color: EncodedRgba::from_bytes([37, 93, 157, alpha], 1.)?,
            },
            Batch {
                first: 6,
                count: 6,
                color: EncodedRgba::from_bytes([1, 7, 19, 128], 1.)?,
            },
        ];
        let actual = renderer.render(&geometry, &batches, Pose::default(), [255; 4])?;
        if let Some(ref p) = output {
            std::fs::write(p.join(format!("{name}.rgba")), &actual).map_err(|e| e.to_string())?;
        }
        let differences = actual
            .chunks_exact(4)
            .zip(reference.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(actual.len(), reference.len());
        println!("Metal source wholeRGBA case={name} different_pixels={differences}");
        assert_eq!(
            differences, 0,
            "Strict actualQPainter whole-image comparison failed"
        );
    }
    assert_eq!(renderer.stats().geometry_uploads, 1);
    assert_eq!(renderer.stats().palette_preparations, 6);
    let color = EncodedRgba::from_bytes([255, 0, 0, 255], 1.)?;
    let batch = [Batch {
        first: 0,
        count: 6,
        color,
    }];
    let pose = Pose {
        scale: [0.5; 2],
        translation: [16.; 2],
    };
    let image = renderer.render(&geometry, &batch, pose, [255; 4])?;
    assert_eq!(
        &image[(24 * 64 + 24) * 4..(24 * 64 + 24) * 4 + 4],
        &[255, 0, 0, 255]
    );
    for _ in 0..10 {
        assert_eq!(renderer.render(&geometry, &batch, pose, [255; 4])?, image);
    }
    assert_eq!(renderer.stats().geometry_uploads, 1);
    assert_eq!(renderer.stats().palette_preparations, 7);
    // Actual vertex allocation followed by index admission refusal; old cached
    // immutable geometry, previous image and accounting remain intact.
    let replacement = Geometry::new(
        vec![
            Vertex {
                position: [2., 2.],
                coverage: 1.
            };
            3
        ],
        vec![0, 1, 2],
    )?;
    let fresh = NativeMetal::new()?;
    let probe = fresh.buffer(36, true)?;
    let vertex_charge = probe.allocation_bytes();
    drop(probe);
    let committed = budget.lock().unwrap().total_committed();
    budget.lock().unwrap().total_limit = Some(committed + vertex_charge);
    assert!(renderer.prepare(&replacement).is_err());
    assert_eq!(budget.lock().unwrap().total_committed(), committed);
    budget.lock().unwrap().total_limit = None;
    assert_eq!(renderer.render(&geometry, &batch, pose, [255; 4])?, image);
    assert_eq!(renderer.stats().geometry_uploads, 1);
    assert!(
        renderer
            .render(
                &geometry,
                &[Batch {
                    first: u32::MAX,
                    ..batch[0]
                }],
                pose,
                [255; 4]
            )
            .is_err()
    );
    println!(
        "Metal CAD stats {:?}; warm unchanged zero geometry/palette preparation; actual candidate index budget denial restored prior image/ledger",
        renderer.stats()
    );
    drop(renderer);
    assert_eq!(budget.lock().unwrap().total_committed(), baseline);
    println!(
        "PASS direct native Metal six actual QPainter wholeRGBA images, warm uniforms/cache, actual admission rollback, final ledger zero. No WSI/full shapeAA/physicalMac/Intel closure"
    );
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Native Metal requires macOS; no fallback");
    std::process::exit(2);
}
