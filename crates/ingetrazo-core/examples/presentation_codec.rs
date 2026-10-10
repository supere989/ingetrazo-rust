use ingetrazo_core::presentation::{DocumentStyle, ShadowSettings};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let input: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let rows: Vec<_> = input["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let result = if r["kind"] == "style" {
                DocumentStyle::try_from_json(&r["input"]).map(|v| v.to_json())
            } else {
                ShadowSettings::try_from_json(&r["input"]).map(|v| v.to_json())
            };
            let (output, error) = match result {
                Ok(v) => (v, serde_json::Value::Null),
                Err(e) => (serde_json::Value::Null, serde_json::json!(e)),
            };
            serde_json::json!({"kind":r["kind"],"input":r["input"],"output":output,"error":error})
        })
        .collect();
    println!("{}", serde_json::json!({"cases":rows}));
}
