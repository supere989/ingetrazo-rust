//! Cold image chooser/header preparation and deferred source Add ownership.
use crate::IngeTrazoApp;
use ingetrazo_tools::ToolType;
use std::{
    io::{Cursor, Read},
    path::Path,
};
#[derive(Default)]
pub struct State {
    asset: Option<Asset>,
    pub error: Option<String>,
    pub preparations: u64,
    pub byte_reads: u64,
    pub committed: u64,
}
struct Asset {
    key: String,
    bytes: Option<Vec<u8>>,
}
impl State {
    pub fn owns_input(&self) -> bool {
        self.error.is_some()
    }
    pub fn retire(&mut self) {
        self.asset = None;
    }
    pub fn pending_bytes(&self) -> usize {
        self.asset
            .as_ref()
            .and_then(|a| a.bytes.as_ref())
            .map_or(0, Vec::len)
    }
}
fn prepare(path: &Path) -> Result<(String, f64, String, Vec<u8>), String> {
    let limit = ingetrazo_render::source_texture_decode::Limits::default().encoded_bytes;
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let size = usize::try_from(f.metadata().map_err(|e| e.to_string())?.len())
        .map_err(|_| "Image input too large")?;
    if size == 0 || size > limit {
        return Err("Image encoded input exceeds the configured resource admission".into());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| "Could not reserve image input")?;
    let mut block = [0u8; 16384];
    loop {
        let n = f.read(&mut block).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if bytes.len().checked_add(n).is_none_or(|n| n > limit) {
            return Err("Image input grew beyond resource admission".into());
        }
        bytes
            .try_reserve(n)
            .map_err(|_| "Could not reserve image input")?;
        bytes.extend_from_slice(&block[..n]);
    }
    let reader = image::ImageReader::new(Cursor::new(bytes.as_slice()))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let (w, h) = reader
        .into_dimensions()
        .map_err(|e| format!("This build cannot read the image header: {e}"))?;
    if w == 0 || h == 0 {
        return Err("Image has no dimensions".into());
    }
    let name = path
        .file_name()
        .ok_or("Image has no filename")?
        .to_string_lossy();
    let hash = blake3::hash(&bytes).to_hex();
    let key = format!("textures/{}-{name}", &hash[..16]);
    let label = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    Ok((key, h as f64 / w as f64, label, bytes))
}
impl IngeTrazoApp {
    pub fn begin_image_import_dialog(&mut self) {
        match crate::dialogs::pick_open_file(
            &ingetrazo_core::i18n::tr("Import image"),
            "*.png *.jpg *.jpeg *.bmp *.tif *.tiff *.webp|Images",
        ) {
            Ok(Some(path)) => {
                if let Err(error) = self.begin_image_import(&path) {
                    self.image_import.error = Some(error);
                }
            }
            Ok(None) => {}
            Err(error) => self.image_import.error = Some(error),
        }
    }
    pub fn begin_image_import(&mut self, path: &Path) -> Result<(), String> {
        if self.navigation_suspended() {
            return Err("Finish the active dialog first".into());
        }
        let (key, aspect, label, bytes) = prepare(path)?;
        let stored = self.scene.embedded_textures.contains_key(&key);
        self.set_tool(ToolType::Image);
        self.active_tool = Box::new(ingetrazo_tools::ImageTool::loaded(
            key.clone(),
            aspect,
            label,
        ));
        self.image_import.asset = Some(Asset {
            key,
            bytes: (!stored).then_some(bytes),
        });
        self.image_import.preparations += 1;
        self.image_import.byte_reads += 1;
        self.status_message=ingetrazo_core::i18n::tr("Click a corner and drag to size the image, or type a width and press Enter (Esc cancels)");
        Ok(())
    }
    pub fn image_placement_label(&self) -> Option<(String, glam::DVec3)> {
        let (w, h, point, free) = self.active_tool.image_size_label()?;
        let (unit, precision) = crate::annotation_plan::label_units(&self.scene).ok()?;
        let mut a = ingetrazo_render::annotations::format_length(w, unit, precision).ok()?;
        let b = ingetrazo_render::annotations::format_length(h, unit, precision).ok()?;
        if matches!(unit, "m" | "cm" | "mm") {
            a = a.strip_suffix(&format!(" {unit}")).unwrap_or(&a).to_owned();
        }
        let mut text = format!("{a} × {b}");
        if free {
            text.push_str("  (libre)");
        }
        Some((text, point))
    }
    pub(crate) fn publish_image_tool_request(&mut self) -> Result<(), String> {
        let Some(plane) = self.active_tool.take_image_creation() else {
            return Ok(());
        };
        let Some(asset) = self.image_import.asset.as_mut() else {
            return Err("Imported image resource owner is missing".into());
        };
        if plane.path != asset.key {
            return Err("Imported image resource identity changed".into());
        }
        self.scene.image_planes.try_reserve(1)?;
        self.scene
            .layers
            .try_reserve(1)
            .map_err(|_| "Could not reserve image layer")?;
        self.scene
            .embedded_textures
            .try_reserve(1)
            .map_err(|_| "Could not reserve image resource")?;
        self.undo_stack
            .try_reserve(1)
            .map_err(|_| "Could not reserve image history")?;
        if let Some(bytes) = asset.bytes.take() {
            self.add_prepared_image_plane(plane, bytes);
        } else {
            self.add_image_plane(plane);
        }
        self.image_import.committed += 1;
        self.bump_model_revision();
        self.active_tool.finish_image_creation();
        self.set_tool(ToolType::Select);
        Ok(())
    }
}
pub(crate) fn render(app: &mut IngeTrazoApp, ctx: &egui::Context) {
    let Some(error) = app.image_import.error.as_deref() else {
        return;
    };
    let response = egui::Modal::new(egui::Id::new("image-import-error")).show(ctx, |ui| {
        ui.heading(ingetrazo_core::i18n::tr("Import image failed"));
        ui.label(error);
        let button = ui.button(ingetrazo_core::i18n::tr("OK"));
        if std::env::var_os("INGETRAZO_UI_OBSERVE").is_some() {
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("image-import-error-ok"), button.rect));
        }
        button.clicked()
    });
    if response.inner || response.should_close() {
        app.image_import.error = None;
    }
}
