//! Dedicated source-compatible DAE export. Export never publishes a document
//! replacement, finishes component editing, changes camera or seals history.
use crate::app::IngeTrazoApp;
use ingetrazo_core::i18n::tr;
use std::path::{Path, PathBuf};
impl IngeTrazoApp {
    pub fn choose_dae_export(&mut self) {
        let base = self
            .current_file
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("untitled");
        let default = format!("{base}.dae");
        if let Some(path) =
            crate::dialogs::pick_save_file(&tr("Export COLLADA"), &default, "*.dae|COLLADA")
        {
            let _ = self.export_dae_to_path(&path);
        }
    }
    pub fn export_dae_to_path(&mut self, chosen: &Path) -> Result<PathBuf, String> {
        let path = if chosen
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("dae"))
        {
            chosen.to_owned()
        } else {
            chosen.with_extension("dae")
        };
        let mut overrides = Vec::new();
        overrides
            .try_reserve_exact(self.component_edit_stack.len())
            .map_err(|_| "DAE edit-view admission failed".to_owned())?;
        for (i, session) in self.component_edit_stack.iter().enumerate() {
            let owner = self
                .component_edit_stack
                .get(i + 1)
                .map_or(&self.scene, |s| s.parked_scene());
            overrides.push(ingetrazo_formats::dae::PlacementOverride {
                id: session.edited_group_id(),
                mesh: &owner.mesh,
                children: &owner.groups,
            });
        }
        let root = self
            .component_edit_stack
            .first()
            .map_or(&self.scene, |s| s.parked_scene());
        match ingetrazo_formats::dae::save_dae_view(
            &self.scene,
            &root.mesh,
            &root.groups,
            &overrides,
            &path,
            "IngeTrazo model",
        ) {
            Ok(()) => {
                self.status_message = format!(
                    "{} → {}",
                    tr("Exported COLLADA"),
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
                Ok(path)
            }
            Err(e) => {
                let message = e.to_string();
                self.dae_export_error = Some(message.clone());
                self.status_message = format!("{}: {message}", tr("Export COLLADA failed"));
                Err(message)
            }
        }
    }
}
pub(crate) fn render(app: &mut IngeTrazoApp, ctx: &egui::Context) {
    let Some(error) = app.dae_export_error.as_ref() else {
        return;
    };
    let mut close = false;
    egui::Window::new(tr("Export COLLADA failed"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label(error);
            if ui.button(tr("OK")).clicked() {
                close = true;
            }
        });
    if close {
        app.dae_export_error = None;
    }
}
