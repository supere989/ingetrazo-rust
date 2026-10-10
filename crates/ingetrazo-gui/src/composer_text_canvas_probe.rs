// SPDX-License-Identifier: GPL-3.0-or-later
//! Actual production Canvas cache/encoded tile observation, not a GPU substitute.
use super::*;
use std::path::Path;
pub(super) fn run(root: &Path) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    if std::fs::read_dir(root)
        .map_err(|e| e.to_string())?
        .next()
        .is_some()
    {
        return Err("Native font tile output must be empty".into());
    }
    let mut resources = Painter::default();
    let mut rows = Vec::new();
    let mut failed = false;
    for (name, family, bold, italic, text) in [
        ("helvetica", "Helvetica", false, false, "ABC"),
        ("helvetica-bold", "Helvetica", true, false, "ABC"),
        ("helvetica-italic", "Helvetica", false, true, "ABC"),
        ("times-bold-italic", "Times", true, true, "ABC"),
        ("cjk", "Helvetica", false, false, "漢字"),
        ("arabic", "Helvetica", false, false, "العربية"),
        ("italic-overlap", "Helvetica", false, true, "AVAVA"),
        ("arabic-marks", "Helvetica", false, false, "العَربيَّة"),
        ("times-overlap", "Times", true, true, "AVWA"),
    ] {
        let result = (|| -> Result<serde_json::Value, String> {
            let ctx = egui::Context::default();
            crate::paper_raw_texture::install(&ctx);
            let mut sheet = Sheet::default();
            let item = TextItem {
                id: uuid::Uuid::new_v4(),
                pos: (10.37, 10.23),
                text: text.into(),
                font_size_pt: 14.,
                bold,
            };
            sheet.native_items.insert(
                item.id,
                serde_json::json!({"family":family,"italic":italic,"color":"#1e242c","w_mm":80.}),
            );
            let mut cache = CanvasPainter::default();
            cache.sample_observation=Some(Vec::new());
            cache.begin_frame();
            let input = || egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 300.))),
                ..Default::default()
            };
            ctx.begin_pass(input());
            let painter = ctx.layer_painter(egui::LayerId::background());
            let bounds = cache.paint(
                &painter,
                &sheet,
                &item,
                text,
                1,
                1,
                3.,
                Pos2::ZERO,
                &mut resources,
            )?;
            let first_output = ctx.end_pass();
            assert!(!first_output.shapes.is_empty());
            let tile = cache.tiles.get(&item.id).ok_or("Missing production tile")?;
            let handle = tile.texture.as_ref().ok_or("Empty production tile")?;
            let id = handle.id();
            let tile_image =
                crate::paper_raw_texture::peek(&ctx, id)?.ok_or("Missing raw payload")?;
            let x = bounds.min.x as i64;
            let y = bounds.min.y as i64;
            if bounds.min.x != x as f32 || bounds.min.y != y as f32 {
                return Err("Tile must align to native device pixels".into());
            }
            let mut frame = vec![255u8; 800 * 300 * 4];
            for row in 0..tile_image.size[1] {
                for col in 0..tile_image.size[0] {
                    let tx = x + col as i64;
                    let ty = y + row as i64;
                    if tx >= 0 && ty >= 0 && tx < 800 && ty < 300 {
                        let dst = &mut frame[((ty * 800 + tx) as usize) * 4..][..4];
                        let src = &tile_image.pixels[(row * tile_image.size[0] + col) * 4..][..4];
                        source_over(dst, src);
                    }
                }
            }
            if !frame.chunks_exact(4).any(|p| p[..3] != [255, 255, 255]) {
                return Err("Empty Canvas ink".into());
            }
            image::save_buffer(
                root.join(format!("{name}.aggregate.png")),
                &frame,
                800,
                300,
                image::ColorType::Rgba8,
            )
            .map_err(|e| e.to_string())?;
            // Observe the actual production cached masks and device placements,
            // rather than generating substitute glyphs or inferred coverage.
            let mut ordered=vec![255u8;800*300*4];let mut masks=Vec::new();
            for (index,sample) in cache.sample_observation.as_ref().unwrap().iter().enumerate(){
                let filename=format!("{name}.mask-{index}.png");
                if sample.image.width>0&&sample.image.height>0{
                    image::save_buffer(root.join(&filename),&sample.image.pixels,sample.image.width as u32,sample.image.height as u32,image::ColorType::Rgba8).map_err(|e|e.to_string())?;
                }
                masks.push(serde_json::json!({"index":index,"position":[sample.x,sample.y],"size":[sample.image.width,sample.image.height],"clip":sample.clip,"mask":filename,"pixel_encoding":"premultiplied RGBA8; compare raw stored bytes without unpremultiply"}));
                for row in 0..sample.image.height{for col in 0..sample.image.width{
                    let x=sample.x+col as i64;let y=sample.y+row as i64;
                    if x>=0&&x<800&&y>=0&&y<300&&x>=sample.clip[0]&&x<sample.clip[2]&&y>=sample.clip[1]&&y<sample.clip[3]{
                        source_over(&mut ordered[((y*800+x)as usize)*4..][..4],&sample.image.pixels[(row*sample.image.width+col)*4..][..4]);
                    }
                }}
            }
            image::save_buffer(root.join(format!("{name}.ordered.png")),&ordered,800,300,image::ColorType::Rgba8).map_err(|e|e.to_string())?;
            std::fs::write(root.join(format!("{name}.masks.json")),serde_json::to_vec_pretty(&masks).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            let producer=ingetrazo_render::cad_callback::Factory::default();
            let factory=crate::composer_backend::Factory::MetalProducer(producer.clone());
            ctx.begin_pass(input());let painter=ctx.layer_painter(egui::LayerId::background());
            let ordered_bounds=cache.paint_with_factory(&painter,&sheet,&item,text,1,1,3.,Pos2::ZERO,&mut resources,Some(&factory))?;
            let output=ctx.end_pass();assert!(!output.shapes.is_empty());assert_eq!(bounds,ordered_bounds);
            let ordered_cache=cache.tiles.get(&item.id).unwrap().ordered.as_ref().unwrap();
            let ordered_records=ordered_cache.observe([800,300],ingetrazo_render::cad2d::Pose{scale:[1.;2],translation:[item.pos.0 as f32*3.,item.pos.1 as f32*3.]},1.)?;
            if ordered_records!=ordered{return Err("Ordered GPU records differ from actual source mask order".into());}
            image::save_buffer(root.join(format!("{name}.native.png")),&ordered_records,800,300,image::ColorType::Rgba8).map_err(|e|e.to_string())?;
            let before_producer=producer.metrics();let ordered_before=cache.ordered_stats();
            let before = (
                cache.stats(),
                cache.tile_builds,
                resources.stats(),
                crate::paper_raw_texture::stats(&ctx),
            );
            for _ in 0..3 {
                cache.begin_frame();
                ctx.begin_pass(input());
                let painter = ctx.layer_painter(egui::LayerId::background());
                let warm = cache.paint_with_factory(
                    &painter,
                    &sheet,
                    &item,
                    text,
                    1,
                    1,
                    3.,
                    Pos2::ZERO,
                    &mut resources,
                    Some(&factory),
                )?;
                assert_eq!(warm, bounds);
                let output = ctx.end_pass();
                assert!(!output.shapes.is_empty());
                assert_eq!(
                    before,
                    (
                        cache.stats(),
                        cache.tile_builds,
                        resources.stats(),
                        crate::paper_raw_texture::stats(&ctx)
                    )
                );
            }
            assert_eq!(cache.ordered_stats(),ordered_before);
            assert_eq!(producer.metrics().handles,before_producer.handles);assert_eq!(producer.metrics().updates,before_producer.updates);
            let layout = cache.layouts.get(&item.id).unwrap().draws.clone();
            let text_run = layout.iter().find_map(|d| d.text.as_ref()).unwrap();
            let phase_counts = text_run
                .glyphs
                .iter()
                .map(|g| {
                    text_run.font.raster_phase_count(
                        g.glyph,
                        text_run.scale_mm * 3.,
                        text_run.bold,
                        text_run.italic,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut style_control = None;
            if name == "times-bold-italic" {
                let layout = cache.layouts.get(&item.id).unwrap().draws.clone();
                let t = layout.iter().find_map(|d| d.text.as_ref()).unwrap();
                let glyph = t.glyphs[0].glyph;
                let scale = t.scale_mm * 3.;
                let regular =
                    cache.image(&t.font, glyph, scale, [0, 0], false, true, Color32::BLACK)?;
                let styled =
                    cache.image(&t.font, glyph, scale, [0, 0], true, true, Color32::BLACK)?;
                assert!(!Arc::ptr_eq(&regular.pixels, &styled.pixels));
                assert_ne!(&*regular.pixels, &*styled.pixels);
                assert!(t
                    .font
                    .rasterize_styled_glyph(u32::MAX, scale, [0, 0], true, true)
                    .is_err());
                assert!(t
                    .font
                    .rasterize_styled_glyph(glyph, f64::NAN, [0, 0], true, true)
                    .is_err());
                let mut scale_controls = Vec::new();
                for scale in [0.25, 1., 4., 32.] {
                    let plain =
                        match t
                            .font
                            .rasterize_styled_glyph(glyph, scale, [0, 0], false, true)
                        {
                            Ok(v) => v,
                            Err(e) if scale == 32. && e == "Native raster byte budget exceeded" => {
                                assert_eq!(
                                    t.font
                                        .rasterize_styled_glyph(glyph, scale, [0, 0], true, true)
                                        .unwrap_err(),
                                    e
                                );
                                scale_controls
                                    .push(serde_json::json!({"scale":scale,"bounded_refusal":e}));
                                continue;
                            }
                            Err(e) => return Err(e),
                        };
                    let bold = t
                        .font
                        .rasterize_styled_glyph(glyph, scale, [0, 0], true, true)?;
                    assert_eq!(
                        (plain.width, plain.height, plain.left, plain.top),
                        (bold.width, bold.height, bold.left, bold.top)
                    );
                    let right_edge_ink = bold
                        .coverage
                        .chunks_exact(bold.width as usize)
                        .filter(|row| row.last().is_some_and(|v| *v > 0))
                        .count();
                    scale_controls.push(serde_json::json!({"scale":scale,"size":[bold.width,bold.height],"right_edge_ink_rows":right_edge_ink,"source_identical_padded_bounds":true}));
                }
                style_control = Some(
                    serde_json::json!({"bold_cache_identity_distinct":true,"missing_bold_changes_pixels":true,"invalid_glyph_and_scale_refused":true,"scale_bounds":scale_controls}),
                );
            }
            Ok(
                serde_json::json!({"name":name,"passed":true,"size":[800,300],"tile_size":tile_image.size,"tile_origin":[x,y],"warm_before":before,"warm_frames":3,"warm_unchanged":true,"style_control":style_control,"phase_counts":phase_counts,"ordered_before":ordered_before,"ordered_warm_unchanged":true,"ordered_handle_count":producer.metrics().handles,"ordered_updates":producer.metrics().updates}),
            )
        })();
        match result {
            Ok(row) => rows.push(row),
            Err(error) => {
                failed = true;
                rows.push(serde_json::json!({"name":name,"passed":false,"error":error}));
            }
        }
    }
    let held_names=["italic-overlap","arabic-marks","times-overlap"];
    let(mut cases,mut held_out)=(Vec::new(),Vec::new());for row in rows{if held_names.contains(&row["name"].as_str().unwrap()){held_out.push(row)}else{cases.push(row)}}
    let report = serde_json::json!({"passed":!failed,"scope":"Actual production native font resolver/layout/raster/ordered cached span records + encoded white byte observation; aggregate negative retained; no GPU upload, WSI, desktop or physical input claim","cases":cases,"held_out_cases":held_out});
    std::fs::write(
        root.join("native-canvas.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    if failed {
        Err("One or more required native Canvas font cases refused".into())
    } else {
        Ok(())
    }
}
