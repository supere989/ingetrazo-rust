// Actual frozen source input recipes, production typed paragraph output; full UI remains open.
use ingetrazo_formats::{
    paper_font_resolver::{Request, Resolver},
    paper_text_layout::{Alignment, ParagraphCache},
    sheet_pdf::{self, Draw, Page, PathOp, TextDecoration, TextDraw, TextGlyph},
};
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let input: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let out = std::path::Path::new(&args[2]);
    std::fs::create_dir_all(out).unwrap();
    let mut resolver = Resolver::new().unwrap();
    let mut cache = ParagraphCache::default();
    for case in input["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let call = &case["source_draw_calls"][0];
        let f = &call["font"];
        let flags = call["flags"].as_u64().unwrap();
        let request = Request {
            family: f["requested"].as_str().unwrap().into(),
            bold: f["bold"].as_bool().unwrap(),
            italic: f["italic"].as_bool().unwrap(),
            required_chars: vec![],
        };
        let paragraph = cache
            .layout(
                &mut resolver,
                &request,
                call["text"].as_str().unwrap(),
                call["rect"][2].as_f64().unwrap(),
                if flags & 4 != 0 {
                    Alignment::Center
                } else if flags & 2 != 0 {
                    Alignment::Right
                } else {
                    Alignment::Left
                },
            )
            .unwrap();
        let scale = 14. * 25.4 / 72. / 100. * 0.75;
        let clip = if paragraph.height_26_6 as f64 / 64. > call["rect"][3].as_f64().unwrap()
            || paragraph.exceeds_width
        {
            let w = call["rect"][2].as_f64().unwrap() * scale;
            let h = call["rect"][3].as_f64().unwrap() * scale;
            Some(vec![
                PathOp::Move([0., 0.]),
                PathOp::Line([w, 0.]),
                PathOp::Line([w, h]),
                PathOp::Line([0., h]),
                PathOp::Close,
            ])
        } else {
            None
        };
        let mut draws = vec![];
        for line in &paragraph.lines {
            let underline_metrics = line.underline_metrics_px;
            for run in &line.line.runs {
                let start = (line.x_26_6 + run.x_26_6) as f64 / 64.;
                let mut x = start;
                let baseline = line.baseline_26_6 as f64 / 64.;
                let mut glyphs = vec![];
                for glyph in &run.glyphs {
                    glyphs.push(TextGlyph {
                        glyph: glyph.id,
                        position_px: [
                            x + glyph.offset[0] as f64 / 64.,
                            baseline + glyph.offset[1] as f64 / 64.,
                        ],
                    });
                    x += glyph.advance[0] as f64 / 64.;
                }
                let mut decorations = vec![];
                if f["underline"].as_bool().unwrap() {
                    let [t, offset] = underline_metrics;
                    decorations.push(TextDecoration {
                        line_px: [
                            start.floor(),
                            baseline + offset,
                            x.floor(),
                            baseline + offset,
                        ],
                        width_px: t,
                    });
                }
                draws.push(Draw {
                    origin_mm: [20., 30.],
                    text: Some(TextDraw {
                        font: run.font.clone(),
                        scale_mm: scale,
                        italic: request.italic,
                        bold: request.bold,
                        glyphs,
                        decorations,
                    }),
                    even_odd: true,
                    path: vec![],
                    stroke: None,
                    stroke_options: None,
                    fill: Some([30f32 / 255., 36f32 / 255., 44f32 / 255., 1.]),
                    clip: clip.clone(),
                    clip_intersection: None,
                    flatten_transform: false,
                });
            }
        }
        let page = Page {
            device_dpi: Some(300.),
            size_mm: [210., 297.],
            media_box_points: Some([595., 842.]),
            origin_mm: [10.08 / (72. / 25.4); 2],
            draws,
        };
        sheet_pdf::save_original_qt_paper(&out.join(format!("{name}.native.pdf")), &[page])
            .unwrap();
    }
}
