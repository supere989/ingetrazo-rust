// SPDX-License-Identifier: GPL-3.0-or-later
//! Actual shared production paragraph painter and direct embedded-font PDF writer.
#[cfg(target_os = "macos")]
fn main() {
    use ingetrazo_formats::{
        paper_text_layout::Alignment,
        paper_text_painter::{Painter, TextStyle},
        sheet_pdf::Page,
    };
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("output directory"));
    std::fs::create_dir_all(&out).unwrap();
    let mut painter = Painter::default();
    let mut records = Vec::new();
    let mut failed = false;
    for (name, family, bold, italic, text) in [
        ("helvetica", "Helvetica", false, false, "ABC"),
        ("helvetica-bold", "Helvetica", true, false, "ABC"),
        ("helvetica-italic", "Helvetica", false, true, "ABC"),
        ("times-bold-italic", "Times", true, true, "ABC"),
        ("cjk", "Helvetica", false, false, "漢字"),
        ("arabic", "Helvetica", false, false, "العربية"),
    ] {
        let style = TextStyle {
            text,
            origin_mm: [10.37, 10.23],
            width_mm: 80.,
            size_pt: 14.,
            family,
            bold,
            italic,
            underline: false,
            alignment: Alignment::Left,
            color: [30. / 255., 36. / 255., 44. / 255., 1.],
        };
        let result = (|| -> Result<serde_json::Value, String> {
            let draws = painter.draw(&style)?;
            let metrics=draws.iter().filter_map(|d|d.text.as_ref()).map(|t|serde_json::json!({"font_key":format!("{:?}",t.font.key()),"scale_mm":t.scale_mm,"bold":t.bold,"italic":t.italic,"glyphs":t.glyphs.iter().map(|g|serde_json::json!({"id":g.glyph,"position_px":g.position_px})).collect::<Vec<_>>()})).collect::<Vec<_>>();
            let physical = 72. / 25.4;
            let page = Page {
                device_dpi: Some(300.),
                size_mm: [210., 297.],
                media_box_points: Some([595., 842.]),
                origin_mm: [10.08 / physical, 10.08 / physical],
                draws,
            };
            let bytes = ingetrazo_formats::sheet_pdf::encode_original_qt_paper(
                std::slice::from_ref(&page),
            )?;
            let stats = painter.stats();
            let again_draws = painter.draw(&style)?;
            assert_eq!(stats, painter.stats());
            let again_page = Page {
                draws: again_draws,
                ..page
            };
            let again = ingetrazo_formats::sheet_pdf::encode_original_qt_paper(&[again_page])?;
            assert_eq!(bytes, again);
            std::fs::write(out.join(format!("{name}.native.pdf")), &bytes)
                .map_err(|e| e.to_string())?;
            Ok(
                serde_json::json!({"name":name,"passed":true,"pdf_bytes":bytes.len(),"draws":metrics,"warm_layout_unchanged":true,"warm_pdf_bytes_exact":true}),
            )
        })();
        match result {
            Ok(row) => records.push(row),
            Err(error) => {
                failed = true;
                records.push(serde_json::json!({"name":name,"passed":false,"error":error}));
            }
        }
    }
    std::fs::write(
        out.join("native-pdf.json"),
        serde_json::to_vec_pretty(&records).unwrap(),
    )
    .unwrap();
    if failed {
        std::process::exit(1)
    }
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Actual CoreText PDF comparison requires macOS");
    std::process::exit(2)
}
