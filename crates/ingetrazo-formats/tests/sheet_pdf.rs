use ingetrazo_formats::sheet_pdf::{self, Draw, Page, PathOp};
fn page() -> Page {
    Page {
        device_dpi: None,
        size_mm: [210., 297.],
        media_box_points: None,
        origin_mm: [0., 0.],
        draws: vec![Draw {
            origin_mm: [0., 0.],
            even_odd: false,
            path: vec![
                PathOp::Move([20., 20.]),
                PathOp::Line([60., 20.]),
                PathOp::Line([60., 45.]),
                PathOp::Line([20., 45.]),
                PathOp::Close,
            ],
            stroke: Some(([0., 0., 0., 1.], 0.35)),
            fill: Some([37. / 255., 147. / 255., 93. / 255., 1.]),
            clip: None,
        }],
    }
}
#[test]
fn invalid_export_preserves_existing_destination() {
    let path = std::env::temp_dir().join(format!("sheet-pdf-{}.pdf", uuid::Uuid::new_v4()));
    std::fs::write(&path, b"existing destination").unwrap();
    let mut p = page();
    p.draws[0].path.push(PathOp::Line([f64::NAN, 0.]));
    assert!(sheet_pdf::save(&path, &[p]).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"existing destination");
    std::fs::remove_file(path).unwrap();
}
#[test]
fn empty_atlas_or_invalid_stroke_is_refused() {
    assert!(sheet_pdf::encode(&[]).is_err());
    let mut p = page();
    p.draws[0].stroke.as_mut().unwrap().1 = 0.;
    assert!(sheet_pdf::encode(&[p]).is_err());
}
#[test]
fn valid_physical_vector_page_contains_no_image_or_process() {
    let bytes = sheet_pdf::encode(&[page()]).unwrap();
    let s = String::from_utf8_lossy(&bytes);
    assert!(s.starts_with("%PDF-"));
    assert!(s.contains("/MediaBox"));
    assert!(s.contains("/ExtGState"));
    assert!(!s.contains("/Subtype /Image"));
}

fn shape_sheet() -> ingetrazo_core::composition::Sheet {
    let mut sheet = ingetrazo_core::composition::Sheet::default();
    sheet.items.clear();
    sheet
}
#[test]
fn visible_unsupported_items_refuse_but_hidden_items_do_not() {
    use ingetrazo_formats::sheet_pdf_shapes::prepare;
    let mut sheet = shape_sheet();
    sheet.native_document = serde_json::json!({"images":[{"hidden":true}]});
    assert!(prepare(&sheet).is_ok());
    sheet.native_document["images"][0]["hidden"] = serde_json::json!(false);
    assert!(prepare(&sheet).err().unwrap().contains("images"));
    assert!(prepare(&ingetrazo_core::composition::Sheet::default()).is_err());
}
#[test]
fn shape_export_keeps_source_visibility_order_and_physical_origin() {
    use ingetrazo_core::{
        composition::{ShapeItem, SheetItem},
        sheet_shape::SheetShape,
    };
    let mut sheet = shape_sheet();
    for (x, z, hidden) in [(70., 9., false), (30., 1., false), (90., 0., true)] {
        sheet.items.push(SheetItem::Shape(ShapeItem {
            id: uuid::Uuid::new_v4(),
            shape: SheetShape {
                x_mm: x,
                z,
                hidden,
                ..Default::default()
            },
        }));
    }
    let p = ingetrazo_formats::sheet_pdf_shapes::prepare(&sheet).unwrap();
    assert_eq!(p.draws.len(), 2);
    assert_eq!(p.draws[0].origin_mm, [30., 20.]);
    assert_eq!(p.draws[1].origin_mm, [70., 20.]);
    assert!(matches!(p.draws[0].path[0], PathOp::Move([0., 0.])));
    assert!(matches!(p.draws[1].path[0], PathOp::Move([0., 0.])));
    assert_eq!(p.origin_mm, [42. * 25.4 / 300.; 2]);
    assert_eq!(p.media_box_points, Some([1191., 842.]));
    assert!(sheet_pdf::encode(&[p]).is_ok());
}

#[test]
fn vector_coordinates_keep_authored_double_precision() {
    let mut p = page();
    p.draws[0].path[0] = PathOp::Move([1.234567890123, 2.345678901234]);
    let bytes = sheet_pdf::encode(&[p]).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("1.234567890123 2.345678901234 m"));
}
