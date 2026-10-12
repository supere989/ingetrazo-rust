//! Actual DWG chooser -> satellite -> retained DXF unit/import owner.
use crate::{i18n::tr, IngeTrazoApp};
use std::path::{Path, PathBuf};
pub struct Report {
    pub warnings: String,
    pub converter_status: Option<i32>,
    pub null_handles: usize,
    pub duplicate_handles: usize,
    pub converted_bytes: usize,
}
fn application_root() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = executable
        .parent()
        .ok_or("Executable directory unavailable")?;
    // Installed native bundle and development checkout use the same satellite layout.
    for p in [
        dir.to_path_buf(),
        dir.join("ingetrazo"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("ingetrazo"),
    ] {
        if p.join("vendor/libredwg/bin").is_dir() {
            return Ok(p);
        }
    }
    Ok(dir.to_path_buf())
}
impl IngeTrazoApp {
    pub fn choose_dwg_import(&mut self) {
        if let Some(path) = crate::dialogs::pick_open_file(&tr("Import DWG"), "*.dwg|AutoCAD DWG") {
            if let Err(error) = self.begin_dwg_import(&path) {
                self.status_message = format!("{}: {error}", tr("Import DWG failed"));
            }
        }
    }
    pub fn begin_dwg_import(&mut self, path: &Path) -> Result<Report, String> {
        let root = application_root()?;
        self.begin_dwg_import_from_root(path, &root)
    }
    /// Same production ingress with an explicit bundle root for deterministic packaging/acceptance.
    pub fn begin_dwg_import_from_root(
        &mut self,
        path: &Path,
        root: &Path,
    ) -> Result<Report, String> {
        let tool = ingetrazo_formats::dwg::find_tool(root, "dwg2dxf")
            .ok_or("LibreDWG (dwg2dxf) is not available")?;
        let result = ingetrazo_formats::dwg::convert(&tool, path, ingetrazo_formats::dwg::TIMEOUT)?;
        let report = Report {
            warnings: result.warnings,
            converter_status: result.status,
            null_handles: result.null_handles,
            duplicate_handles: result.duplicate_handles,
            converted_bytes: result.bytes.len(),
        };
        self.begin_unit_import_bytes(path, "dxf", result.bytes)?;
        self.status_message = format!("{} {}", tr("Importing"), path.display());
        Ok(report)
    }
}
