use ingetrazo_formats::{
    paper_font_resolver::{Request, Resolver},
    paper_text_layout::{Alignment, ParagraphCache},
};
fn main() {
    let a = std::env::args().collect::<Vec<_>>();
    let source: serde_json::Value = serde_json::from_slice(&std::fs::read(&a[1]).unwrap()).unwrap();
    let mut resolver = Resolver::new().unwrap();
    let mut cache = ParagraphCache::default();
    let mut rows = vec![];
    for case in source["cases"].as_array().unwrap() {
        let props = &case["properties"];
        let text = case["authored_text"].as_str().unwrap();
        let req = Request {
            family: props["family"].as_str().unwrap_or("Sans Serif").into(),
            bold: props["bold"].as_bool().unwrap_or(false),
            italic: false,
            required_chars: vec![],
        };
        let font = resolver.resolve(req.clone()).unwrap().font;
        let scale = 14. * 25.4 / 72. / 100. * 0.75;
        let para = cache
            .layout(
                &mut resolver,
                &req,
                text,
                props["w_mm"].as_f64().unwrap_or(80.) / scale,
                Alignment::Left,
            )
            .unwrap();
        let h = (font.ascent_26_6() + font.descent_26_6()) as f64 / 64.;
        let count = if text.is_empty() {
            0.
        } else {
            (para.height_26_6 as f64 / 64. / h)
                .round_ties_even()
                .max(1.)
        };
        rows.push(serde_json::json!({"name":case["name"],"native":{"ascent":font.ascent_26_6()as f64/64.,"descent":font.descent_26_6()as f64/64.,"cap_height":font.cap_height_26_6()as f64/64.,"leading":font.leading_26_6()as f64/64.,"line_spacing":h+font.leading_26_6()as f64/64.,"line_count":count},"source":case["source_background"]}));
    }
    std::fs::write(&a[2], serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
}
