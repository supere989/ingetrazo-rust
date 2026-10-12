//! Test-consumer material identity observer; keeps UV/scale/all metadata literal.
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(crate) fn normalize(app: &IngeTrazoApp, value: &mut Value) -> Result<(), String> {
    match value {
        Value::Object(map) => {
            if let Some(Value::Object(texture)) = map.get_mut("texture") {
                let embed = texture.get("embed").and_then(Value::as_str);
                let path = texture.get("path").and_then(Value::as_str);
                if embed.is_some() && path.is_some() {
                    return Err("Ambiguous material resource locator".into());
                }
                if let Some(key) = embed.or(path).map(str::to_owned) {
                    let owned;
                    let bytes = if let Some(bytes) = app.scene.embedded_textures.get(&key) {
                        bytes.as_slice()
                    } else {
                        if embed.is_some() {
                            return Err(format!("Missing embedded material resource {key}"));
                        }
                        owned = std::fs::read(&key).map_err(|e| e.to_string())?;
                        owned.as_slice()
                    };
                    let pixels =
                        ingetrazo_render::source_texture_decode::decode(bytes, Default::default())?;
                    let observed = json!({"encoded_sha256":sha(bytes),"rgba_sha256":sha(&pixels.rgba),"size":[pixels.width,pixels.height]});
                    texture.remove("embed");
                    texture.insert("path".into(), observed);
                }
            }
            for child in map.values_mut() {
                normalize(app, child)?;
            }
        }
        Value::Array(array) => {
            for child in array {
                normalize(app, child)?;
            }
        }
        _ => {}
    }
    Ok(())
}
