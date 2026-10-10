use ingetrazo_formats::sheet_pdf::{self, Draw, Page, PathOp};
fn page(size: [f64; 2]) -> Page {
    Page {
        device_dpi: None,
        size_mm: size,
        media_box_points: None,
        origin_mm: [10. * 25.4 / 72.; 2],
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
            stroke: Some(([7. / 255., 19. / 255., 25. / 255., 1.], 0.35)),
            fill: Some([37. / 255., 147. / 255., 93. / 255., 1.]),
            stroke_options: None,
            clip: None,
            clip_intersection: None,
        }],
    }
}
fn main() -> Result<(), String> {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    sheet_pdf::save(&dir.join("native-vector.pdf"), &[page([210., 297.])])?;
    sheet_pdf::save(
        &dir.join("native-atlas.pdf"),
        &[page([210., 297.]), page([420., 297.]), page([1189., 841.])],
    )?;
    Ok(())
}
