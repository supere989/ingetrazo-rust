// Experimental layout expansion: input recipes only; output derived by native fonts, retained failures mandatory.
use ingetrazo_formats::{
    paper_font_resolver::{Request, Resolver},
    sheet_pdf::{self, Draw, Page, TextDecoration, TextDraw, TextGlyph},
};
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let out = std::path::Path::new(&args[2]);
    std::fs::create_dir_all(out).unwrap();
    let mut resolver = Resolver::new().unwrap();
    let mut metrics = Vec::new();
    for case in source["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let call = &case["source_draw_calls"][0];
        let f = &call["font"];
        let matched = resolver
            .resolve(Request {
                family: f["requested"].as_str().unwrap().into(),
                bold: f["bold"].as_bool().unwrap(),
                italic: f["italic"].as_bool().unwrap(),
                required_chars: vec![],
            })
            .unwrap();
        let font = matched.font;
        let text = call["text"].as_str().unwrap();
        let width = (call["rect"][2].as_f64().unwrap() * 64.).floor() / 64.;
        let flags = call["flags"].as_u64().unwrap();
        let advance = |s: &str| {
            font.shape_run(s, None)
                .unwrap()
                .iter()
                .map(|g| g.advance[0] as i64)
                .sum::<i64>()
        };
        let mut lines = Vec::new();
        for para in text.split('\n') {
            let mut current = String::new();
            for word in para.split(' ') {
                let next = if current.is_empty() {
                    word.into()
                } else {
                    format!("{current} {word}")
                };
                if !current.is_empty() && advance(&next) as f64 / 64. > width {
                    lines.push(current);
                    current = word.into();
                } else {
                    current = next;
                }
            }
            lines.push(current);
        }
        let line_height = ((font.ascent_26_6() + font.descent_26_6()) as f64 / 64.).ceil();
        let mut glyphs = Vec::new();
        let mut decorations = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let a = advance(line) as f64 / 64.;
            let x = if flags & 4 != 0 {
                (width - a) / 2.
            } else if flags & 2 != 0 {
                width - a
            } else {
                0.
            };
            let baseline = font.ascent_26_6() as f64 / 64. + i as f64 * line_height;
            let mut cursor = x;
            for g in font.shape_run(line, None).unwrap() {
                glyphs.push(TextGlyph {
                    glyph: g.id,
                    position_px: [
                        cursor + g.offset[0] as f64 / 64.,
                        baseline + g.offset[1] as f64 / 64.,
                    ],
                });
                cursor += g.advance[0] as f64 / 64.;
            }
            if f["underline"].as_bool().unwrap() {
                let [t, offset] = font.underline_metrics_px().unwrap();
                decorations.push(TextDecoration {
                    line_px: [
                        x.floor(),
                        baseline + offset,
                        (x + a).floor(),
                        baseline + offset,
                    ],
                    width_px: t,
                });
            }
        }
        metrics.push(serde_json::json!({"case":name,"lines":lines,"line_height":line_height,"underline":decorations.iter().map(|d|serde_json::json!({"line":d.line_px,"width":d.width_px})).collect::<Vec<_>>()}));
        let page = Page {
            device_dpi: Some(300.),
            size_mm: [210., 297.],
            media_box_points: Some([595., 842.]),
            origin_mm: [10.08 / (72. / 25.4); 2],
            draws: vec![Draw {
                origin_mm: [20., 30.],
                text: Some(TextDraw {
                    font,
                    scale_mm: 14. * 25.4 / 72. / 100. * 0.75,
                    italic: f["italic"].as_bool().unwrap(),
                    glyphs,
                    decorations,
                }),
                even_odd: true,
                path: vec![],
                stroke: None,
                stroke_options: None,
                fill: Some([30f32 / 255., 36f32 / 255., 44f32 / 255., 1.]),
                clip: None,
                clip_intersection: None,
                flatten_transform: false,
            }],
        };
        sheet_pdf::save_original_qt_paper(&out.join(format!("{name}.native.pdf")), &[page])
            .unwrap();
    }
    std::fs::write(
        out.join("native-layout-metrics.json"),
        serde_json::to_vec_pretty(&metrics).unwrap(),
    )
    .unwrap();
}
