// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(not(target_os = "macos"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ingetrazo_gui::native_vulkan::world::World;
    let path = std::env::args().nth(1).ok_or("input required")?;
    let input: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let mut rows = Vec::new();
    for row in input["rows"].as_array().ok_or("rows")? {
        let d = serde_json::from_value(row["direction"].clone())?;
        let lo = serde_json::from_value(row["lo"].clone())?;
        let hi = serde_json::from_value(row["hi"].clone())?;
        rows.push(serde_json::json!({"matrix":World::shadow_light_matrix_for_probe(d,lo,hi)?}));
    }
    println!("{}", serde_json::to_string_pretty(&rows)?);
    Ok(())
}
#[cfg(target_os = "macos")]
fn main() {
    eprintln!("Direct Vulkan diagnostic is Linux/Windows only");
}
