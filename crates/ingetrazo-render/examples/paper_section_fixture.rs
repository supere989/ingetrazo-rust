use glam::DVec3;
fn vector(v: &serde_json::Value) -> DVec3 {
    DVec3::from_array(std::array::from_fn(|i| v[i].as_f64().unwrap()))
}
fn triangles(v: &serde_json::Value) -> Vec<[DVec3; 3]> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|t| std::array::from_fn(|i| vector(&t[i])))
        .collect()
}
fn main() {
    let source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(std::env::args().nth(1).unwrap()).unwrap()).unwrap();
    for case in source["loops"].as_array().unwrap() {
        let cuts = case["cuts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| (vector(&e[0]), vector(&e[1])))
            .collect::<Vec<_>>();
        let expected = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_array().unwrap().iter().map(vector).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        assert_eq!(
            ingetrazo_render::paper_section::closed_loops(&cuts),
            expected,
            "source closed loop {}",
            case["name"]
        );
    }
    for case in source["clips"].as_array().unwrap() {
        let mut plane = ingetrazo_core::section::SectionPlane::new(
            vector(&case["point"]),
            vector(&case["normal"]),
            "test",
            "T",
        );
        plane.normal = vector(&case["normal"]);
        let (kept, cuts) =
            ingetrazo_render::paper_section::clip_triangles(triangles(&case["triangles"]), &plane);
        let expected = case["expected_cuts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| (vector(&e[0]), vector(&e[1])))
            .collect::<Vec<_>>();
        assert_eq!(
            kept,
            triangles(&case["expected_triangles"]),
            "source clipped triangles {}",
            case["name"]
        );
        assert_eq!(cuts, expected, "source cut chords {}", case["name"]);
    }
    for case in source["axes"].as_array().unwrap() {
        let (u, v) = ingetrazo_render::paper_section::plane_axes(vector(&case["normal"]));
        assert_eq!(
            [u, v],
            [vector(&case["expected"][0]), vector(&case["expected"][1])],
            "source Qt plane axes {:?}",
            case["normal"]
        );
    }
    assert_eq!(source["loops"].as_array().unwrap().len(), 7);
    assert_eq!(source["clips"].as_array().unwrap().len(), 4);
    println!("PASS 7 actual source loops +4 exact source triangle clips +7 Qt plane axes; no coordinate tolerance");
}
