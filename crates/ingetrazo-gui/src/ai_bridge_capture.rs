//! Frontend capture admission/PNG publication; renderer owns GPU targets/fences.
use ingetrazo_core::ai_bridge::DeferredContext;
use serde_json::{json, Value};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
    time::Instant,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureStamp {
    pub document_id: uuid::Uuid,
    pub model_revision: u64,
    pub scene_version: u64,
    pub edit_context: Vec<uuid::Uuid>,
    pub camera_bits: [u64; 15],
    pub logical_size_bits: [u32; 2],
}
impl CaptureStamp {
    pub fn logical_size(&self) -> [f32; 2] {
        self.logical_size_bits.map(f32::from_bits)
    }
    pub fn for_app(app: &crate::IngeTrazoApp) -> Result<Self, String> {
        if app.component_edit_stack.len() > 1024 {
            return Err("Capture edit context admission refused".into());
        }
        let mut edit_context = Vec::new();
        edit_context
            .try_reserve_exact(app.component_edit_stack.len())
            .map_err(|_| "Capture context allocation refused")?;
        edit_context.extend(app.component_edit_stack.iter().map(|s| s.edited_group_id()));
        let viewport = app
            .native_gpu_viewport_rect
            .ok_or("Capture viewport layout unavailable")?;
        let size = [viewport.width(), viewport.height()];
        if !size.iter().all(|v| v.is_finite() && *v > 0.) {
            return Err("Capture logical viewport size refused".into());
        }
        let c = &app.camera;
        Ok(Self {
            document_id: app.clipboard_document,
            model_revision: app.model_revision,
            scene_version: app.scene.version,
            logical_size_bits: size.map(f32::to_bits),
            edit_context,
            camera_bits: [
                c.target.x.to_bits(),
                c.target.y.to_bits(),
                c.target.z.to_bits(),
                c.distance.to_bits(),
                c.yaw.to_bits(),
                c.pitch.to_bits(),
                c.up.x.to_bits(),
                c.up.y.to_bits(),
                c.up.z.to_bits(),
                c.fov_deg.to_bits(),
                c.aspect.to_bits(),
                c.znear.to_bits(),
                c.zfar.to_bits(),
                u64::from(c.perspective),
                u64::from(c.two_point),
            ],
        })
    }
    pub fn matches(&self, app: &crate::IngeTrazoApp) -> bool {
        self.document_id == app.clipboard_document
            && self.model_revision == app.model_revision
            && self.scene_version == app.scene.version
            && self.edit_context.len() == app.component_edit_stack.len()
            && self
                .edit_context
                .iter()
                .copied()
                .eq(app.component_edit_stack.iter().map(|s| s.edited_group_id()))
            && app.native_gpu_viewport_rect.is_some_and(|r| {
                self.logical_size_bits == [r.width().to_bits(), r.height().to_bits()]
            })
            && self.camera_bits == {
                let c = &app.camera;
                [
                    c.target.x.to_bits(),
                    c.target.y.to_bits(),
                    c.target.z.to_bits(),
                    c.distance.to_bits(),
                    c.yaw.to_bits(),
                    c.pitch.to_bits(),
                    c.up.x.to_bits(),
                    c.up.y.to_bits(),
                    c.up.z.to_bits(),
                    c.fov_deg.to_bits(),
                    c.aspect.to_bits(),
                    c.znear.to_bits(),
                    c.zfar.to_bits(),
                    u64::from(c.perspective),
                    u64::from(c.two_point),
                ]
            }
    }
}
#[derive(Clone, Debug)]
pub struct OwnedCapture {
    pub id: u64,
    pub width: u32,
    pub height: u32,
    pub overlays: bool,
    pub deadline: Instant,
    pub stamp: CaptureStamp,
}
pub struct CapturedRgba {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
pub struct CaptureResult {
    pub id: u64,
    pub result: Result<CapturedRgba, String>,
}
pub struct PendingCapture {
    request: OwnedCapture,
    wire_id: Value,
    context: DeferredContext,
}
impl PendingCapture {
    pub fn begin(
        app: &crate::IngeTrazoApp,
        id: u64,
        wire_id: Value,
        args: Option<&Value>,
        context: DeferredContext,
    ) -> Result<Self, String> {
        let (width, height) = dimensions(args)?;
        if !context.is_live() {
            return Err("Capture request cancelled or expired".into());
        }
        Ok(Self {
            request: OwnedCapture {
                id,
                width,
                height,
                overlays: true,
                deadline: context.deadline,
                stamp: CaptureStamp::for_app(app)?,
            },
            wire_id,
            context,
        })
    }
    pub fn request(&self) -> &OwnedCapture {
        &self.request
    }
    pub fn is_live(&self, app: &crate::IngeTrazoApp) -> bool {
        self.context.is_live() && self.request.stamp.matches(app)
    }
    /// Called only after the backend has retired/quarantined its submission.
    pub fn finish(self, app: &crate::IngeTrazoApp, result: CaptureResult) -> Value {
        let output = (|| {
            if result.id != self.request.id {
                return Err("Capture completion identity mismatch".into());
            }
            if !self.is_live(app) {
                return Err("Capture retired after cancellation or document/view change".into());
            }
            let image = result.result?;
            if [image.width, image.height] != [self.request.width, self.request.height] {
                return Err("Capture extent mismatch".into());
            }
            let bytes = (image.width as usize)
                .checked_mul(image.height as usize)
                .and_then(|n| n.checked_mul(4))
                .ok_or("Capture byte size overflow")?;
            if image.rgba.len() != bytes {
                return Err("Capture RGBA inventory mismatch".into());
            }
            let folder = std::env::temp_dir().join("ingetrazo-ai");
            std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
            let target = folder.join("viewport.png");
            let staging = folder.join(format!(
                ".viewport-{}-{}.png.tmp",
                std::process::id(),
                self.request.id
            ));
            let encoded = (|| {
                use image::ImageEncoder;
                let file = File::create(&staging).map_err(|e| e.to_string())?;
                let mut writer = BufWriter::new(file);
                image::codecs::png::PngEncoder::new(&mut writer)
                    .write_image(
                        &image.rgba,
                        image.width,
                        image.height,
                        image::ExtendedColorType::Rgba8,
                    )
                    .map_err(|e| e.to_string())?;
                writer.flush().map_err(|e| e.to_string())?;
                drop(writer);
                if !self.is_live(app) {
                    return Err("Capture cancelled before PNG publication".into());
                }
                std::fs::rename(&staging, &target).map_err(|e| e.to_string())?;
                Ok::<PathBuf, String>(target)
            })();
            if encoded.is_err() {
                let _ = std::fs::remove_file(&staging);
            }
            let target = encoded?;
            Ok::<Value, String>(
                json!({"path":target.to_string_lossy(),"width":image.width,"height":image.height}),
            )
        })();
        match output {
            Ok(value) => json!({"id":self.wire_id,"ok":true,"result":value}),
            Err(error) => json!({"id":self.wire_id,"ok":false,"error":error}),
        }
    }
}
fn dimension(value: &Value) -> Result<u32, String> {
    let number = match value {
        Value::Null => return Err("TypeError: int() argument must be a string, a bytes-like object or a real number, not 'NoneType'".into()),
        Value::Bool(v) => {
            if *v {
                1.
            } else {
                0.
            }
        }
        Value::Number(v) => v
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or("Capture dimension must be finite")?
            .trunc(),
        Value::String(v) => {
            let text = v.trim();
            let (negative, digits) = if let Some(x) = text.strip_prefix('-') {
                (true, x)
            } else {
                (false, text.strip_prefix('+').unwrap_or(text))
            };
            if digits.is_empty()
                || !digits.bytes().all(|c| c.is_ascii_digit() || c == b'_')
                || digits.starts_with('_')
                || digits.ends_with('_')
                || digits.contains("__")
            {
                return Err("Capture dimension must be a decimal integer".into());
            }
            let mut n = 0u32;
            for digit in digits.bytes().filter(|b| *b != b'_') {
                n = n.saturating_mul(10).saturating_add(u32::from(digit - b'0'));
            }
            if negative {
                -(n as f64)
            } else {
                n as f64
            }
        }
        _ => return Err("Capture dimension cannot be converted to integer".into()),
    };
    Ok(number.clamp(64., 4096.) as u32)
}
pub fn dimensions(args: Option<&Value>) -> Result<(u32, u32), String> {
    let Some(args) = args else {
        return Ok((1024, 768));
    };
    // Original req.get(args) or {} also treats these empty/falsy values as defaults.
    if args.is_null()
        || args == &json!(false)
        || args == &json!(0)
        || args == &json!("")
        || args == &json!([])
    {
        return Ok((1024, 768));
    }
    let fields = args
        .as_object()
        .ok_or("Screenshot args must be an object")?;
    if let Some(keyword) = fields
        .keys()
        .find(|k| k.as_str() != "width" && k.as_str() != "height")
    {
        return Err(format!(
            "TypeError: _Bridge._tool_screenshot() got an unexpected keyword argument '{keyword}'"
        ));
    }
    Ok((
        fields
            .get("width")
            .map(dimension)
            .transpose()?
            .unwrap_or(1024),
        fields
            .get("height")
            .map(dimension)
            .transpose()?
            .unwrap_or(768),
    ))
}

#[cfg(test)]
mod tests {
    use super::dimensions;
    use serde_json::json;
    #[test]
    fn screenshot_dimensions_clamp_integer_conversion_without_accepting_decimal_text() {
        assert_eq!(dimensions(None).unwrap(), (1024, 768));
        assert_eq!(
            dimensions(Some(&json!({"width":-2,"height":999999}))).unwrap(),
            (64, 4096)
        );
        assert_eq!(
            dimensions(Some(&json!({"width":120.9,"height":" 1_024 "}))).unwrap(),
            (120, 1024)
        );
        assert_eq!(
            dimensions(Some(&json!({"width":true,"height":false}))).unwrap(),
            (64, 64)
        );
        assert!(dimensions(Some(&json!({"width":"1.2"}))).is_err());
        assert_eq!(dimensions(Some(&json!({"height":null}))).unwrap_err(), "TypeError: int() argument must be a string, a bytes-like object or a real number, not 'NoneType'");
        assert_eq!(
            dimensions(Some(&json!({"unexpected":1}))).unwrap_err(),
            "TypeError: _Bridge._tool_screenshot() got an unexpected keyword argument 'unexpected'"
        );
    }
}
