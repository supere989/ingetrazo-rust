use ingetrazo_formats::{
    paper_font_resolver::{Request, Resolver},
    paper_text::PaperFont,
};
fn main() -> Result<(), String> {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/audits/native-annotation-source/original.json"
    ))
    .map_err(|e| e.to_string())?;
    assert_eq!(
        reference["source_pin"],
        "6be29fe437a7cd992d4e079f71720821b25216b9"
    );
    let mut resolver = Resolver::new()?;
    let mut checked = 0;
    for row in reference["cases"].as_array().unwrap() {
        for metrics in row["font_metrics"].as_array().unwrap() {
            let info = &metrics["font_info"];
            let matched = resolver.resolve(Request {
                family: info["family"].as_str().unwrap().into(),
                bold: info["weight"].as_u64().unwrap() >= 700,
                italic: false,
                required_chars: vec![],
            })?;
            let font = PaperFont::new_at_pixel_size(
                matched.font.shared_bytes().clone(),
                matched.index,
                Some(matched.policy),
                info["pixel_size"].as_u64().unwrap() as u32,
            )?;
            let paper = PaperFont::new_with_outline_policy(
                matched.font.shared_bytes().clone(),
                matched.index,
                Some(matched.policy),
            )?;
            let baseline = PaperFont::new_at_pixel_size(
                matched.font.shared_bytes().clone(),
                matched.index,
                Some(matched.policy),
                100,
            )?;
            let ascent = (font.ascent_26_6 + 63) / 64;
            let descent = (font.descent_26_6 + 63) / 64;
            assert_eq!(
                ascent,
                i32::try_from(metrics["ascent"].as_i64().unwrap()).unwrap()
            );
            assert_eq!(
                descent,
                i32::try_from(metrics["descent"].as_i64().unwrap()).unwrap()
            );
            assert_eq!(
                ascent + descent,
                i32::try_from(metrics["height"].as_i64().unwrap()).unwrap()
            );
            for advance in metrics["advances"].as_array().unwrap() {
                let text = advance["text"].as_str().unwrap();
                assert_eq!(
                    paper.shape_run(text, None)?,
                    baseline.shape_run(text, None)?
                );
                let glyphs = font.shape_run(text, None)?;
                let width: i64 = glyphs.iter().map(|g| i64::from(g.advance[0])).sum();
                assert_eq!(
                    (width as f64 / 64. + 0.5).floor() as i64,
                    advance["value"].as_i64().unwrap(),
                    "{text}"
                );
                for glyph in glyphs {
                    let _ = font.rasterize_glyph(glyph.id, 1., [0, 0], 0.)?;
                }
                checked += 1;
            }
            assert!(PaperFont::new_at_pixel_size(
                matched.font.shared_bytes().clone(),
                matched.index,
                Some(matched.policy),
                0
            )
            .is_err());
            assert!(PaperFont::new_at_pixel_size(
                matched.font.shared_bytes().clone(),
                matched.index,
                Some(matched.policy),
                u32::MAX
            )
            .is_err());
        }
    }
    assert!(checked >= 10);
    println!(
        "actual resolved source font metric controls passed: {checked}; pixel100 shaping unchanged"
    );
    Ok(())
}
