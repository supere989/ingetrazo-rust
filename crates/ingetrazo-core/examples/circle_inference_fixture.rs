use glam::DVec3;
fn main() {
    let paths: Vec<_> = std::env::args().collect();
    let cases: Vec<Vec<[f64; 3]>> =
        serde_json::from_slice(&std::fs::read(&paths[1]).unwrap()).unwrap();
    let result: Vec<_> = cases
        .iter()
        .map(|ps| {
            ingetrazo_core::circle_inference::fit_circle(
                &ps.iter().map(|p| DVec3::from_array(*p)).collect::<Vec<_>>(),
            )
            .map(|(c, r)| (c.to_array(), r))
        })
        .collect();
    std::fs::write(&paths[2], serde_json::to_vec(&result).unwrap()).unwrap();
}
