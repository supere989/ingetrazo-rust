//! Original live DimensionStylePanel: nonundoable document presentation.
use crate::IngeTrazoApp;
#[derive(Clone, Debug, PartialEq)]
pub struct Controls {
    pub decimals: i32,
    pub units: String,
    pub font_size: i32,
    pub color: [u8; 3],
    pub ends: String,
    pub base_step_mm: f64,
}
pub const UNITS: &[&str] = &[
    "m",
    "cm",
    "mm",
    "in",
    "ft",
    "ft-in",
    "in-frac",
    "ft-in-frac",
];
impl IngeTrazoApp {
    pub fn dimension_style_controls(&self) -> Controls {
        let s = self.scene.dimension_style();
        let units = s["units"]
            .as_str()
            .filter(|x| UNITS.contains(x))
            .unwrap_or("m");
        let ends = s["ends"]
            .as_str()
            .filter(|x| ["arrow", "tick", "none"].contains(x))
            .unwrap_or("arrow");
        Controls {
            decimals: s["decimals"].as_i64().unwrap_or(2).clamp(0, 4) as i32,
            units: units.into(),
            font_size: s["font_size"].as_i64().unwrap_or(9).clamp(6, 28) as i32,
            color: std::array::from_fn(|i| {
                s["color"][i]
                    .as_u64()
                    .and_then(|n| u8::try_from(n).ok())
                    .unwrap_or([45, 55, 75][i])
            }),
            ends: ends.into(),
            base_step_mm: s["base_step_mm"]
                .as_f64()
                .filter(|n| n.is_finite())
                .unwrap_or(8.)
                .clamp(1., 60.),
        }
    }
    pub fn apply_dimension_style(&mut self, c: Controls) -> Result<(), String> {
        if !(0..=4).contains(&c.decimals)
            || !(6..=28).contains(&c.font_size)
            || !UNITS.contains(&c.units.as_str())
            || !["arrow", "tick", "none"].contains(&c.ends.as_str())
            || !c.base_step_mm.is_finite()
            || !(1. ..=60.).contains(&c.base_step_mm)
        {
            return Err("Dimension style controls outside source range".into());
        }
        let mut raw = match self.scene.dimension_style() {
            serde_json::Value::Object(map) => serde_json::Value::Object(map.clone()),
            serde_json::Value::Null => {
                serde_json::json!({"decimals":2,"units":"m","font_size":9,"color":[45,55,75],"norma":"iso","base_step_mm":8.,"ends":"arrow"})
            }
            _ => return Err("Dimension style must be an object".into()),
        };
        raw["decimals"] = c.decimals.into();
        raw["units"] = c.units.into();
        raw["font_size"] = c.font_size.into();
        raw["color"] = serde_json::json!(c.color);
        raw["ends"] = c.ends.into();
        raw["base_step_mm"] = serde_json::json!(c.base_step_mm);
        self.scene.dimension_style_override = Some(raw);
        self.scene.bump_version();
        self.bump_model_revision();
        Ok(())
    }
}
pub(crate) fn render(app: &mut IngeTrazoApp, ui: &mut egui::Ui) {
    let mut c = app.dimension_style_controls();
    let mut changed = false;
    egui::Grid::new("source_dimension_style").show(ui, |ui| {
        ui.label("Decimals:");
        changed |= ui
            .add(egui::DragValue::new(&mut c.decimals).range(0..=4))
            .changed();
        ui.end_row();
        ui.label("Unit:");
        egui::ComboBox::from_id_salt("annotation_units")
            .selected_text(&c.units)
            .show_ui(ui, |ui| {
                for unit in UNITS {
                    changed |= ui
                        .selectable_value(&mut c.units, (*unit).into(), *unit)
                        .changed();
                }
            });
        ui.end_row();
        ui.label("Font:");
        changed |= ui
            .add(egui::DragValue::new(&mut c.font_size).range(6..=28))
            .changed();
        ui.end_row();
        ui.label("Color:");
        changed |= ui.color_edit_button_srgb(&mut c.color).changed();
        ui.end_row();
        ui.label("Ends:");
        egui::ComboBox::from_id_salt("annotation_ends")
            .selected_text(&c.ends)
            .show_ui(ui, |ui| {
                for (label, value) in [
                    ("Arrows", "arrow"),
                    ("Oblique ticks", "tick"),
                    ("None", "none"),
                ] {
                    changed |= ui
                        .selectable_value(&mut c.ends, value.into(), label)
                        .changed();
                }
            });
        ui.end_row();
        ui.label("Baseline step:");
        changed |= ui
            .add(
                egui::DragValue::new(&mut c.base_step_mm)
                    .range(1. ..=60.)
                    .speed(0.5)
                    .max_decimals(1)
                    .suffix(" mm"),
            )
            .changed();
        ui.end_row();
    });
    if changed {
        if let Err(error) = app.apply_dimension_style(c) {
            app.status_message = error;
        }
    }
}
