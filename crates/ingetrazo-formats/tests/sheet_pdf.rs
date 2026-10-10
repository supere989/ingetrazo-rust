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
            stroke_options: None,
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

#[test]
fn preparation_admits_aggregate_paths_before_returning_large_page() {
    use ingetrazo_core::{
        composition::{ShapeItem, SheetItem},
        sheet_shape::SheetShape,
    };
    let mut sheet = shape_sheet();
    sheet.items.push(SheetItem::Shape(ShapeItem {
        id: uuid::Uuid::new_v4(),
        shape: SheetShape {
            kind: "terreno".into(),
            ground: "hatch".into(),
            w_mm: 40_000.,
            h_mm: 0.,
            tick_step_mm: 0.5,
            band_mm: 1.,
            ..Default::default()
        },
    }));
    assert!(ingetrazo_formats::sheet_pdf_shapes::prepare(&sheet)
        .err()
        .unwrap()
        .contains("aggregate command budget"));
}
#[test]
fn failed_destination_rename_leaves_directory_and_removes_temporary() {
    let parent = std::env::temp_dir().join(format!("sheet-pdf-directory-{}", uuid::Uuid::new_v4()));
    let destination = parent.join("existing");
    std::fs::create_dir_all(&destination).unwrap();
    std::fs::write(destination.join("sentinel"), b"unchanged").unwrap();
    assert!(sheet_pdf::save(&destination, &[page()]).is_err());
    assert_eq!(
        std::fs::read(destination.join("sentinel")).unwrap(),
        b"unchanged"
    );
    assert_eq!(std::fs::read_dir(&parent).unwrap().count(), 1);
    std::fs::remove_dir_all(parent).unwrap();
}

#[test]
fn numeric_serialization_preserves_legacy_boundary_bytes() {
    fn legacy(x: f64) -> String {
        let s = format!("{x:.12}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        if s == "-0" {
            "0".into()
        } else {
            s.into()
        }
    }
    let mut values = vec![
        0.,
        -0.,
        f64::MIN_POSITIVE,
        -f64::MIN_POSITIVE,
        5e-13,
        -5e-13,
        1e-12,
        -1e-12,
        999999.9999999999,
        -999999.9999999999,
        1_000_000.,
        -1_000_000.,
    ];
    for i in 0..2048 {
        values.push((i as f64 - 1024.) * 976.123456789012);
    }
    let mut p = page();
    p.draws[0].path = values.iter().map(|&x| PathOp::Move([x, -x])).collect();
    let bytes = sheet_pdf::encode(&[p]).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    for x in values {
        assert!(
            text.contains(&format!("{} {} m\n", legacy(x), legacy(-x))),
            "{x}"
        );
    }
}
#[test]
fn preparation_rejects_total_draws_across_individually_admitted_shapes() {
    use ingetrazo_core::{
        composition::{ShapeItem, SheetItem},
        sheet_shape::SheetShape,
    };
    let mut sheet = shape_sheet();
    for _ in 0..2 {
        sheet.items.push(SheetItem::Shape(ShapeItem {
            id: uuid::Uuid::new_v4(),
            shape: SheetShape {
                kind: "terreno".into(),
                ground: "ticks".into(),
                w_mm: 30_000.,
                h_mm: 0.,
                tick_step_mm: 0.5,
                band_mm: 1.,
                ..Default::default()
            },
        }));
    }
    assert!(ingetrazo_formats::sheet_pdf_shapes::prepare(&sheet)
        .err()
        .unwrap()
        .contains("draw budget"));
}

#[test]
fn atlas_preparation_refuses_combined_draws_before_retaining_all_pages() {
    use ingetrazo_core::{
        composition::{ShapeItem, SheetItem},
        sheet_shape::SheetShape,
    };
    let mut sheet = shape_sheet();
    sheet.items.push(SheetItem::Shape(ShapeItem {
        id: uuid::Uuid::new_v4(),
        shape: SheetShape {
            kind: "terreno".into(),
            ground: "ticks".into(),
            w_mm: 30_000.,
            h_mm: 0.,
            tick_step_mm: 0.5,
            band_mm: 1.,
            ..Default::default()
        },
    }));
    assert!(ingetrazo_formats::sheet_pdf_shapes::prepare(&sheet).is_ok());
    assert!(
        ingetrazo_formats::sheet_pdf_shapes::prepare_all(&[sheet.clone(), sheet])
            .err()
            .unwrap()
            .contains("atlas draw budget")
    );
}

#[test]
fn original_qt_paper_numerals_are_explicit_and_generic_precision_is_retained() {
    let mut p = page();
    p.draws[0].path = vec![
        PathOp::Move([55.11536795523, -0.1234567899]),
        PathOp::Line([-1e-10, 999.9999999999]),
    ];
    let native =
        String::from_utf8_lossy(&sheet_pdf::encode(std::slice::from_ref(&p)).unwrap()).into_owned();
    let qt = String::from_utf8_lossy(
        &sheet_pdf::encode_original_qt_paper(std::slice::from_ref(&p)).unwrap(),
    )
    .into_owned();
    assert!(native.contains("55.11536795523 -0.1234567899 m"));
    assert!(qt.contains("55.1153679 -0.123456789 m"));
    assert!(qt.contains("-0 999.999999 l"));
    assert_ne!(native, qt);
}
#[test]
fn invalid_qt_paper_export_preserves_destination_and_rejects_stroke_options() {
    let dest = std::env::temp_dir().join(format!("qt-paper-{}.pdf", uuid::Uuid::new_v4()));
    std::fs::write(&dest, b"original").unwrap();
    for options in [
        sheet_pdf::StrokeOptions {
            cap: 3,
            join: 1,
            dash: vec![],
        },
        sheet_pdf::StrokeOptions {
            cap: 0,
            join: 1,
            dash: vec![f64::NAN],
        },
        sheet_pdf::StrokeOptions {
            cap: 0,
            join: 1,
            dash: vec![0.],
        },
    ] {
        let mut p = page();
        p.draws[0].stroke_options = Some(options);
        assert!(sheet_pdf::save_original_qt_paper(&dest, &[p]).is_err());
        assert_eq!(std::fs::read(&dest).unwrap(), b"original");
    }
    std::fs::remove_file(dest).unwrap();
}
