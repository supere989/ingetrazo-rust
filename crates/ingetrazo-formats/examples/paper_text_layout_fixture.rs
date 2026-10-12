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
        let mut font = matched.font;
        let text = call["text"].as_str().unwrap();
        if font
            .shape_run(text, None)
            .unwrap()
            .iter()
            .any(|g| g.id == 0)
        {
            font = resolver
                .resolve(Request {
                    family: f["requested"].as_str().unwrap().into(),
                    bold: f["bold"].as_bool().unwrap(),
                    italic: f["italic"].as_bool().unwrap(),
                    required_chars: text.chars().collect(),
                })
                .unwrap()
                .font;
        }
        let width = (call["rect"][2].as_f64().unwrap() * 64.).floor() / 64.;
        let flags = call["flags"].as_u64().unwrap();
        let shape = |s: &str| {
            let bidi = unicode_bidi::BidiInfo::new(s, None);
            let mut output = Vec::new();
            for para in &bidi.paragraphs {
                let (levels, runs) = bidi.visual_runs(para, para.range.clone());
                for run in runs {
                    output.extend(
                        font.shape_run(&s[run.clone()], Some(levels[run.start].is_rtl()))
                            .unwrap(),
                    );
                }
            }
            output
        };
        let advance = |s: &str| shape(s).iter().map(|g| g.advance[0] as i64).sum::<i64>();
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
                (((width - a) * 64.) / 2.).floor() / 64.
            } else if flags & 2 != 0 {
                width - a
            } else {
                0.
            };
            let baseline = font.ascent_26_6() as f64 / 64. + i as f64 * line_height;
            let mut cursor = x;
            for g in shape(line) {
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
            draws: vec![Draw { rotation_deg:0.,
                origin_mm: [20., 30.],
                text: Some(TextDraw {
                        rotation_deg: 0.,
                    font: font.clone(),
                    scale_mm: 14. * 25.4 / 72. / 100. * 0.75,
                    italic: f["italic"].as_bool().unwrap(),
                    bold: f["bold"].as_bool().unwrap(),
                    glyphs,
                    decorations,
                }),
                even_odd: true,
                path: vec![],
                stroke: None,
                stroke_options: None,
                fill: Some([30f32 / 255., 36f32 / 255., 44f32 / 255., 1.]),
                clip: if line_height * lines.len() as f64 > call["rect"][3].as_f64().unwrap()
                    || lines.iter().any(|l| advance(l) as f64 / 64. > width)
                {
                    let w = call["rect"][2].as_f64().unwrap() * 14. * 25.4 / 72. / 100. * 0.75;
                    let h = call["rect"][3].as_f64().unwrap() * 14. * 25.4 / 72. / 100. * 0.75;
                    Some(vec![
                        sheet_pdf::PathOp::Move([0., 0.]),
                        sheet_pdf::PathOp::Line([w, 0.]),
                        sheet_pdf::PathOp::Line([w, h]),
                        sheet_pdf::PathOp::Line([0., h]),
                        sheet_pdf::PathOp::Close,
                    ])
                } else {
                    None
                },
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
