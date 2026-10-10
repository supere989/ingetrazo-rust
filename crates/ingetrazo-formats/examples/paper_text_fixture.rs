use std::sync::Arc;
fn main() {
    let source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(std::env::args().nth(1).unwrap()).unwrap()).unwrap();
    let mut count = 0;
    for invalid in [
        Vec::new(),
        b"not a font".to_vec(),
        vec![0u8; ingetrazo_formats::paper_text::MAX_FONT_BYTES + 1],
    ] {
        assert!(ingetrazo_formats::paper_text::PaperFont::new(invalid.into(), 0).is_err());
    }
    for case in source["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if ![
            "latin",
            "accents",
            "bold",
            "italic",
            "underline",
            "serif",
            "mono",
            "combining",
            "kerns",
            "serif_bold",
        ]
        .contains(&name)
        {
            continue;
        }
        let call = &case["source_draw_calls"][0];
        let bytes: Arc<[u8]> = std::fs::read(call["font"]["file"].as_str().unwrap())
            .unwrap()
            .into();
        let weak = Arc::downgrade(&bytes);
        let font = ingetrazo_formats::paper_text::PaperFont::new(bytes.clone(), 0).unwrap();
        assert!(Arc::ptr_eq(font.shared_bytes(), &bytes));
        drop(bytes); // font must keep the READONLY blob storage alive
        assert!(font
            .shape_run(
                &"x".repeat(ingetrazo_formats::paper_text::MAX_TEXT_BYTES + 1),
                None
            )
            .is_err());
        assert!(font.shape_run("", None).unwrap().is_empty());
        let text = call["text"].as_str().unwrap();
        let run = font.shape_run(text, None).unwrap();
        assert_eq!(run, font.shape_run(text, None).unwrap(), "warm reused face");
        let expected = &call["supplementary_qtextlayout"]["runs"][0];
        assert_eq!(
            run.len(),
            expected["glyphs"].as_array().unwrap().len(),
            "{name}"
        );
        let mut x = 0i64;
        let ascent = font.ascent_26_6 as f64 / 64.;
        assert_eq!(
            ascent,
            call["font"]["ascent"].as_f64().unwrap(),
            "source ascent {name}"
        );
        assert_eq!(
            font.descent_26_6 as f64 / 64.,
            call["font"]["descent"].as_f64().unwrap(),
            "source descent {name}"
        );
        assert_eq!(
            font.leading_26_6 as f64 / 64.,
            call["font"]["leading"].as_f64().unwrap(),
            "source leading {name}"
        );
        for (i, g) in run.iter().enumerate() {
            assert_eq!(
                g.id as u64,
                expected["glyphs"][i].as_u64().unwrap(),
                "glyph {name}"
            );
            assert_eq!(
                [
                    (x + g.offset[0] as i64) as f64 / 64.,
                    ascent + g.offset[1] as f64 / 64.
                ],
                [
                    expected["positions"][i][0].as_f64().unwrap(),
                    expected["positions"][i][1].as_f64().unwrap()
                ],
                "position {name}"
            );
            x += g.advance[0] as i64;
        }
        drop(font);
        assert!(
            weak.upgrade().is_none(),
            "font bytes released after owner drop"
        );
        count += 1;
    }
    let expanded = source["cases"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["name"] == "combining");
    assert_eq!(count, if expanded { 10 } else { 7 });
    println!("PASS seven actual Qt supplementary glyph runs exact native fixedadvance shaping; PDF/layout still OPEN");
}
