// Bounded direct writer/font cache acceptance, actual app layout integration remains separate.
use ingetrazo_formats::{
    paper_font::{FontCache, FontKey},
    paper_text::{FtPolicy, Hinting},
    sheet_pdf::{Draw, Page, TextDraw, TextGlyph},
};
use std::{collections::HashMap, sync::Arc};
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let out = std::path::Path::new(&args[2]);
    std::fs::create_dir_all(out).unwrap();
    let mut cache = FontCache::default();
    let mut keys = HashMap::new();
    let mut pages = Vec::new();
    let policy = FtPolicy {
        hinting: Hinting::Vertical,
        force_auto_hint: false,
    };
    let mut records = Vec::new();
    for case in source["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if ![
            "accents",
            "bold",
            "combining",
            "italic",
            "kerns",
            "latin",
            "mono",
            "serif",
            "serif_bold",
        ]
        .contains(&name)
        {
            continue;
        }
        let call = &case["source_draw_calls"][0];
        let path = call["font"]["file"].as_str().unwrap();
        let key = *keys.entry(path.to_owned()).or_insert_with(|| FontKey {
            identity: uuid::Uuid::new_v4(),
            face_index: 0,
            policy,
        });
        let font = if let Some(font) = cache.get(key) {
            font
        } else {
            cache
                .load(key, std::fs::read(path).unwrap().into())
                .unwrap()
        };
        let glyphs = font
            .shape_run(call["text"].as_str().unwrap(), None)
            .unwrap();
        let mut x = 0i64;
        let glyphs = glyphs
            .into_iter()
            .map(|g| {
                let position_px = [
                    (x + g.offset[0] as i64) as f64 / 64.,
                    (font.ascent_26_6() as i64 + g.offset[1] as i64) as f64 / 64.,
                ];
                x += g.advance[0] as i64;
                TextGlyph {
                    glyph: g.id,
                    position_px,
                }
            })
            .collect();
        let draw = Draw { rotation_deg:0.,
            origin_mm: [20., 30.],
            text: Some(TextDraw {
                        rotation_deg: 0.,
                bold: false,
                font: font.clone(),
                scale_mm: 14. * 25.4 / 72. / 100. * 0.75,
                italic: call["font"]["italic"].as_bool().unwrap(),
                glyphs,
                decorations: Vec::new(),
            }),
            even_odd: true,
            path: Vec::new(),
            stroke: None,
            stroke_options: None,
            fill: Some([30f32 / 255., 36f32 / 255., 44f32 / 255., 1.]),
            clip: None,
            clip_intersection: None,
            flatten_transform: false,
        };
        let physical = 72. / 25.4;
        let page = Page {
            device_dpi: Some(300.),
            size_mm: [210., 297.],
            media_box_points: Some([595., 842.]),
            origin_mm: [10.08 / physical, 10.08 / physical],
            draws: vec![draw],
        };
        let bytes =
            ingetrazo_formats::sheet_pdf::encode_original_qt_paper(std::slice::from_ref(&page))
                .unwrap();
        std::fs::write(out.join(format!("{name}.native.pdf")), &bytes).unwrap();
        let stats = font.stats();
        let again =
            ingetrazo_formats::sheet_pdf::encode_original_qt_paper(std::slice::from_ref(&page))
                .unwrap();
        assert_eq!(bytes, again, "warm PDF bytes exact");
        assert_eq!(stats, font.stats(), "warm no glyph/subset rebuild");
        records.push((name.to_owned(), font.clone(), stats));
        pages.push(page);
    }
    assert_eq!(pages.len(), 9);
    let loads = cache.stats().2;
    for page in &pages {
        let _ = ingetrazo_formats::sheet_pdf::encode_original_qt_paper(std::slice::from_ref(page))
            .unwrap();
    }
    assert_eq!(loads, cache.stats().2);
    let font = pages[0].draws[0].text.as_ref().unwrap().font.clone();
    let original = font.shared_bytes().clone();
    assert!(cache
        .load(font.key(), Arc::from(b"not a font".as_slice()))
        .is_err());
    assert!(Arc::ptr_eq(&original, font.shared_bytes()));
    let sentinel = out.join("refusal-sentinel.pdf");
    std::fs::write(&sentinel, b"preserved").unwrap();
    let mut bad = pages[0].draws[0].clone();
    bad.text.as_mut().unwrap().glyphs[0].position_px[0] = f64::NAN;
    let bad_page = Page {
        device_dpi: Some(300.),
        size_mm: [210., 297.],
        media_box_points: None,
        origin_mm: [0., 0.],
        draws: vec![bad],
    };
    let before = font.stats();
    assert!(ingetrazo_formats::sheet_pdf::save_original_qt_paper(&sentinel, &[bad_page]).is_err());
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"preserved");
    assert_eq!(before, font.stats());
    let mut invalid_decoration = Page {
        device_dpi: pages[0].device_dpi,
        size_mm: pages[0].size_mm,
        media_box_points: pages[0].media_box_points,
        origin_mm: pages[0].origin_mm,
        draws: pages[0].draws.clone(),
    };
    invalid_decoration.draws[0]
        .text
        .as_mut()
        .unwrap()
        .decorations
        .push(ingetrazo_formats::sheet_pdf::TextDecoration {
            line_px: [0., 0., 10., 0.],
            width_px: f64::NAN,
        });
    let before = font.stats();
    assert!(
        ingetrazo_formats::sheet_pdf::save_original_qt_paper(&sentinel, &[invalid_decoration])
            .is_err()
    );
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"preserved");
    assert_eq!(before, font.stats());
    let mut other_cache = FontCache::default();
    let different = pages
        .iter()
        .find(|p| {
            p.draws[0]
                .text
                .as_ref()
                .unwrap()
                .font
                .shared_bytes()
                .as_ref()
                != font.shared_bytes().as_ref()
        })
        .unwrap()
        .draws[0]
        .text
        .as_ref()
        .unwrap()
        .font
        .shared_bytes()
        .clone();
    let impostor = other_cache.load(font.key(), different).unwrap();
    let mut colliding = Page {
        device_dpi: pages[0].device_dpi,
        size_mm: pages[0].size_mm,
        media_box_points: pages[0].media_box_points,
        origin_mm: pages[0].origin_mm,
        draws: pages[0].draws.clone(),
    };
    let mut duplicate = colliding.draws[0].clone();
    duplicate.text.as_mut().unwrap().font = impostor;
    colliding.draws.push(duplicate);
    assert!(ingetrazo_formats::sheet_pdf::save_original_qt_paper(&sentinel, &[colliding]).is_err());
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"preserved");
    let mut invalid = Page {
        device_dpi: pages[0].device_dpi,
        size_mm: pages[0].size_mm,
        media_box_points: pages[0].media_box_points,
        origin_mm: pages[0].origin_mm,
        draws: pages[0].draws.clone(),
    };
    invalid.draws[0].text.as_mut().unwrap().glyphs[0].glyph = u32::MAX;
    assert!(ingetrazo_formats::sheet_pdf::save_original_qt_paper(&sentinel, &[invalid]).is_err());
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"preserved");
    let ids = pages[0].draws[0]
        .text
        .as_ref()
        .unwrap()
        .glyphs
        .iter()
        .map(|g| g.glyph)
        .collect::<Vec<_>>();
    let resource = font.subset(&ids).unwrap();
    assert!(Arc::ptr_eq(&resource, &font.subset(&ids).unwrap()));
    let mut capacity = FontCache::default();
    let mut first = None;
    for _ in 0..17 {
        let mut key = font.key();
        key.identity = uuid::Uuid::new_v4();
        let f = capacity.load(key, font.shared_bytes().clone()).unwrap();
        if first.is_none() {
            first = Some(f);
        }
    }
    assert_eq!(capacity.stats().0, 16);
    let first = first.unwrap();
    assert!(capacity.get(first.key()).is_none());
    assert!(!first.shape_run("retained", None).unwrap().is_empty());
    cache.clear();
    assert_eq!(cache.stats().0, 0);
    let retained = ingetrazo_formats::sheet_pdf::encode_original_qt_paper(&pages[..1]).unwrap();
    assert!(
        !retained.is_empty(),
        "retained page font survives cache retirement"
    );
    println!("PASS nine direct typed font pages; warm same bytes/stats, identity refusal, invalid position atomic, retired cache resource lifetime");
    std::fs::write(out.join("cache-stats.json"),serde_json::to_vec_pretty(&serde_json::json!({"cold_font_loads":loads,"records":records.iter().map(|(name,font,_)|serde_json::json!({"case":name,"glyphs":font.stats().glyphs,"points":font.stats().points,"subset_builds":font.stats().builds,"subset_bytes":font.stats().subset_bytes})).collect::<Vec<_>>(),"scope":"Typed fonts/direct writer only, paragraph/app UI still OPEN"})).unwrap()).unwrap();
}
