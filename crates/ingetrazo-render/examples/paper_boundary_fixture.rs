use glam::{DMat4, DVec3};
use ingetrazo_core::camera::OrbitCamera;
use ingetrazo_render::vector_hlr::OriginalQtPaperBasis;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let corpus = std::path::Path::new(&args[1]);
    let source: serde_json::Value = serde_json::from_slice(
        &std::fs::read(corpus.join("original-paper-boundary.json")).unwrap(),
    )
    .unwrap();
    let mut checked = 0;
    let mut rejects_f64_eye = false;
    let mut rejects_walk_for_bulk = false;
    for row in source["rows"].as_array().unwrap() {
        let reference: serde_json::Value = serde_json::from_slice(
            &std::fs::read(
                corpus
                    .join("corpus")
                    .join(format!("{}.reference.json", row["name"].as_str().unwrap())),
            )
            .unwrap(),
        )
        .unwrap();
        let c = &reference["camera"];
        let v = |key: &str| {
            DVec3::new(
                c[key][0].as_f64().unwrap(),
                c[key][1].as_f64().unwrap(),
                c[key][2].as_f64().unwrap(),
            )
        };
        let mut camera = OrbitCamera::default();
        camera.target = v("target");
        camera.up = v("up");
        camera.yaw = c["yaw"].as_f64().unwrap();
        camera.pitch = c["pitch"].as_f64().unwrap();
        camera.distance = c["distance"].as_f64().unwrap();
        let basis = OriginalQtPaperBasis::from_camera(&camera);
        rejects_f64_eye |= (basis.eye - camera.eye()).length() > 1e-7;
        for (index, vector) in [basis.eye, basis.right, basis.up, basis.forward]
            .iter()
            .enumerate()
        {
            for axis in 0..3 {
                let expected = row["basis"][index][axis].as_f64().unwrap();
                assert!(
                    (vector[axis] - expected).abs() < 2e-15,
                    "{} basis {} axis {}: {} vs {}",
                    row["name"],
                    index,
                    axis,
                    vector[axis],
                    expected
                );
            }
        }
        assert_eq!(
            basis.eye.to_array(),
            [
                row["eye"][0].as_f64().unwrap(),
                row["eye"][1].as_f64().unwrap(),
                row["eye"][2].as_f64().unwrap()
            ]
        );
        for placement in row["placements"].as_array().unwrap() {
            let mut values = [0.; 16];
            for i in 0..16 {
                values[i] = placement["matrix"][i].as_f64().unwrap();
            }
            let matrix = DMat4::from_cols_array(&values);
            for (index, point) in placement["points"].as_array().unwrap().iter().enumerate() {
                let v = DVec3::new(
                    point[0].as_f64().unwrap(),
                    point[1].as_f64().unwrap(),
                    point[2].as_f64().unwrap(),
                );
                rejects_walk_for_bulk |=
                    ingetrazo_render::vector_hlr::original_qt_paper_point(matrix, v, false)
                        != ingetrazo_render::vector_hlr::original_qt_paper_point(matrix, v, true);
                let wrong = ingetrazo_render::vector_hlr::original_qt_paper_point(
                    DMat4::from_translation(DVec3::new(0.125, 0., 0.)) * matrix,
                    v,
                    true,
                );
                assert_ne!(
                    wrong,
                    ingetrazo_render::vector_hlr::original_qt_paper_point(matrix, v, true),
                    "altered placement must be detected"
                );
                for (bulk, field) in [(false, "walk"), (true, "bulk")] {
                    let actual =
                        ingetrazo_render::vector_hlr::original_qt_paper_point(matrix, v, bulk);
                    for axis in 0..3 {
                        assert_eq!(
                            actual[axis],
                            placement[field][index][axis].as_f64().unwrap(),
                            "{} {} point {} axis {}",
                            row["name"],
                            field,
                            index,
                            axis
                        );
                    }
                }
            }
        }
        checked += 1;
    }
    assert!(
        rejects_walk_for_bulk,
        "actual controls must reject Qt walk as substitute for cached affine endpoints"
    );
    assert!(
        rejects_f64_eye,
        "negative control must distinguish unchanged f64 renderer eye"
    );
    println!(
        "PASS {checked} actual original paper camera bases (eye exact; f64 normalization 2e-15)"
    );
}
