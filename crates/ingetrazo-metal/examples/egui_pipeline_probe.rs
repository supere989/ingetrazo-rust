//! Shader/cache/admission controls only: this does not submit or sample a draw.
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ingetrazo_metal::{NativeMetal, egui_painter::EguiPainter};
    let metal = NativeMetal::new()?;
    eprintln!(
        "Native Metal egui pipeline: device={}; unified={}",
        metal.info.name, metal.info.unified_memory
    );
    let mut painter = EguiPainter::new(&metal)?;
    let image = egui::ColorImage::new([2, 2], egui::Color32::WHITE);
    let delta = egui::TexturesDelta {
        set: vec![(
            egui::TextureId::Managed(42),
            egui::epaint::ImageDelta::full(image, egui::TextureOptions::NEAREST),
        )],
        free: vec![],
    };
    painter.update_textures(&metal, &delta)?;
    let vertex = |x, y| egui::epaint::Vertex {
        pos: egui::pos2(x, y),
        uv: egui::pos2(0.25, 0.25),
        color: egui::Color32::WHITE,
    };
    let input = |offset| {
        vec![egui::ClippedPrimitive {
            clip_rect: egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(64., 64.)),
            primitive: egui::epaint::Primitive::Mesh(egui::epaint::Mesh {
                indices: vec![0, 1, 2],
                vertices: vec![
                    vertex(offset, 0.),
                    vertex(32. + offset, 0.),
                    vertex(offset, 32.),
                ],
                texture_id: egui::TextureId::Managed(42),
            }),
        }]
    };
    let prepared = painter.prepare(&metal, &input(0.), 1.)?;
    let stats = painter.stats();
    for _ in 0..10 {
        let _ = painter.prepare(&metal, &input(0.), 1.)?;
    }
    assert_eq!(painter.stats().geometry_uploads, stats.geometry_uploads);
    assert_eq!(painter.stats().texture_uploads, stats.texture_uploads);
    let before = metal.budget.lock().unwrap().total_committed();
    let measured = metal.buffer(60, true)?.allocation_bytes();
    {
        metal.budget.lock().unwrap().total_limit = Some(before + measured);
    }
    assert!(
        painter.prepare(&metal, &input(1.), 1.).is_err(),
        "Real second-buffer admission must fail"
    );
    assert_eq!(metal.budget.lock().unwrap().total_committed(), before);
    assert_eq!(painter.stats().geometry_uploads, stats.geometry_uploads);
    metal.budget.lock().unwrap().total_limit = None;
    let _ = painter.prepare(&metal, &input(0.), 1.)?;
    let bad = egui::TexturesDelta {
        set: vec![(
            egui::TextureId::Managed(42),
            egui::epaint::ImageDelta::partial(
                [2, 0],
                egui::ColorImage::new([1, 1], egui::Color32::RED),
                egui::TextureOptions::NEAREST,
            ),
        )],
        free: vec![],
    };
    assert!(painter.update_textures(&metal, &bad).is_err());
    assert_eq!(metal.budget.lock().unwrap().total_committed(), before);
    let partial = egui::TexturesDelta {
        set: vec![(
            egui::TextureId::Managed(42),
            egui::epaint::ImageDelta::partial(
                [1, 0],
                egui::ColorImage::new([1, 1], egui::Color32::RED),
                egui::TextureOptions::LINEAR,
            ),
        )],
        free: vec![],
    };
    painter.update_textures(&metal, &partial)?;
    let _ = painter.prepare(&metal, &input(0.), 1.)?;
    let mut missing = input(0.);
    if let egui::epaint::Primitive::Mesh(m) = &mut missing[0].primitive {
        m.texture_id = egui::TextureId::User(999);
    }
    assert!(painter.prepare(&metal, &missing, 1.).is_err());
    let callback = egui::ClippedPrimitive {
        clip_rect: egui::Rect::EVERYTHING,
        primitive: egui::epaint::Primitive::Callback(egui::epaint::PaintCallback {
            rect: egui::Rect::EVERYTHING,
            callback: std::sync::Arc::new(()),
        }),
    };
    assert!(painter.prepare(&metal, &[callback], 1.).is_err());
    drop(prepared);
    drop(painter);
    assert_eq!(metal.budget.lock().unwrap().total_committed(), 0);
    eprintln!(
        "PASS: native MSL/pipeline; warm uploads0; actual VB/IB admission rollback; partial metadata; missing texture/callback refusal; ledger0. No sampled draw proof."
    );
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Native Metal requires macOS");
    std::process::exit(2);
}
