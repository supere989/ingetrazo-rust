use ingetrazo_formats::sheet_pdf::{self, Draw, Page, PathOp};
fn main() {
    let input = std::env::args().nth(1).unwrap();
    let source: serde_json::Value = serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
    for row in source["rows"].as_array().unwrap() {
        let v = row["value"].as_f64().unwrap();
        let page = Page {
            size_mm: [210., 297.],
            media_box_points: None,
            device_dpi: None,
            origin_mm: [0., 0.],
            draws: vec![Draw {picture:None, rotation_deg:0.,
                origin_mm: [0., 0.],
                even_odd: false,
                path: vec![PathOp::Move([v, 20.]), PathOp::Line([10., 21.])],
                stroke: Some(([0., 0., 0., 1.], 1.)),
                stroke_options: None,
                fill: None,
                clip: None,
                clip_intersection: None,
                text: None,
                flatten_transform: false,
            }],
        };
        let expected = format!("{} 20 m", row["encoded"].as_str().unwrap());
        let bytes = sheet_pdf::encode_original_qt_paper(std::slice::from_ref(&page)).unwrap();
        assert!(
            String::from_utf8_lossy(&bytes).contains(&expected),
            "actual Qt numeral {v} expected {expected}"
        );
    }
    println!("PASS eight actual Qt PDF numeral controls");
}
