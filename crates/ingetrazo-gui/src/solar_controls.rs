//! Document solar controls. Source panel uses a fixed non-leap 2026 study bar;
//! the rendering clock uses the current native LOCAL calendar year.
use crate::IngeTrazoApp;
use ingetrazo_core::{i18n::tr, presentation::ShadowSettings, sun};
const PANEL_YEAR: i32 = 2026;
const MONTH_DAYS: [u16; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
pub(crate) fn scene_direction(app: &IngeTrazoApp) -> Result<Option<[f64; 3]>, String> {
    if !app.shadows_enabled
        || app.scene.active_sheet_index.is_some()
        || std::env::var_os("INGETRAZO_NO_SHADOWS").is_some_and(|v| !v.is_empty())
    {
        return Ok(None);
    }
    let year = crate::composer_text_fields::sample_calendar()?.year();
    let mut settings = app.scene.shadows.clone();
    settings.enabled = app.shadows_enabled;
    // Source hand-edited invalid dates suppress shadows; the stored data survives.
    Ok(sun::scene_direction(&settings, app.scene.geo_datum.as_ref(), year).unwrap_or(None))
}
fn day_of_year(month: u8, day: u8) -> u16 {
    if !(1..=12).contains(&month) || day == 0 || u16::from(day) > MONTH_DAYS[usize::from(month - 1)]
    {
        return 80;
    }
    MONTH_DAYS[..usize::from(month - 1)].iter().sum::<u16>() + u16::from(day)
}
fn month_day(day: u16) -> (u8, u8) {
    let mut day = day.clamp(1, 365);
    for (month, n) in MONTH_DAYS.iter().enumerate() {
        if day <= *n {
            return ((month + 1) as u8, day as u8);
        }
        day -= n;
    }
    unreachable!()
}
#[derive(Clone, PartialEq, Eq)]
struct DaylightKey {
    lat: u64,
    lon: u64,
    month: u8,
    day: u8,
    offset: Option<i32>,
    invalid_offset: bool,
}
#[derive(Clone)]
struct DaylightCache {
    key: DaylightKey,
    range: Option<(u16, u16)>,
}
fn daylight(ui: &egui::Ui, lat: f64, lon: f64, s: &ShadowSettings) -> Option<(u16, u16)> {
    let id = egui::Id::new("solar_panel_daylight");
    let key = DaylightKey {
        lat: lat.to_bits(),
        lon: lon.to_bits(),
        month: s.month,
        day: s.day,
        offset: s.utc_offset,
        invalid_offset: s.source_utc_offset.is_some(),
    };
    if let Some(c) = ui.ctx().data(|d| d.get_temp::<DaylightCache>(id)) {
        if c.key == key {
            return c.range;
        }
    }
    let range = if key.invalid_offset {
        None
    } else {
        sun::daylight_minutes(lat, lon, s.month, s.day, s.utc_offset, PANEL_YEAR, 5)
            .ok()
            .flatten()
    };
    ui.ctx()
        .data_mut(|d| d.insert_temp(id, DaylightCache { key, range }));
    range
}
fn apply(app: &mut IngeTrazoApp, mut s: ShadowSettings, lat: f64, lon: f64) {
    s.source_utc_offset = None;
    if let Ok(Some((lo, hi))) =
        sun::daylight_minutes(lat, lon, s.month, s.day, s.utc_offset, PANEL_YEAR, 5)
    {
        let m = (u16::from(s.hour) * 60 + u16::from(s.minute)).clamp(lo, hi);
        s.hour = (m / 60) as u8;
        s.minute = (m % 60) as u8;
    }
    app.set_shadow_settings(s);
}
pub(crate) fn show(app: &mut IngeTrazoApp, ui: &mut egui::Ui) {
    ui.separator();
    let mut s = app.scene.shadows.clone();
    s.enabled = app.shadows_enabled;
    let (lat, lon) = app
        .scene
        .geo_datum
        .as_ref()
        .map_or((sun::DEFAULT_LAT, sun::DEFAULT_LON), |d| (d.lat, d.lon));
    let mut changed = ui.checkbox(&mut s.enabled, tr("Show shadows")).changed();
    let initial = daylight(ui, lat, lon, &s);
    let (lo, hi) = initial.unwrap_or((0, 1439));
    // Display clamping is local. Merely opening or repainting never rewrites a file.
    let mut minutes = (u16::from(s.hour) * 60 + u16::from(s.minute)).clamp(lo, hi);
    let mut darkness = ((1. - s.darkness) * 100.).round_ties_even().clamp(0., 80.) as u8;
    if s.source_utc_offset.is_some() || s.utc_offset.is_some_and(|v| !(-12..=14).contains(&v)) {
        s.utc_offset = None;
    }
    egui::Grid::new("solar_clock_grid").show(ui, |ui| {
        ui.label(tr("Date:"));
        ui.horizontal(|ui| {
            let date_changed = ui
                .add(
                    egui::DragValue::new(&mut s.day)
                        .range(1..=31)
                        .prefix("Day "),
                )
                .changed()
                | ui.add(
                    egui::DragValue::new(&mut s.month)
                        .range(1..=12)
                        .prefix("Month "),
                )
                .changed();
            if date_changed {
                s.day = s.day.min(MONTH_DAYS[usize::from(s.month - 1)] as u8);
                changed = true;
            }
        });
        ui.end_row();
        ui.label(tr("Year study:"));
        let mut doy = day_of_year(s.month, s.day);
        if ui
            .add(egui::Slider::new(&mut doy, 1..=365).show_value(false))
            .changed()
        {
            (s.month, s.day) = month_day(doy);
            changed = true;
        }
        ui.end_row();
        ui.label(tr("Time:"));
        changed |= ui
            .add(
                egui::Slider::new(&mut minutes, lo..=hi)
                    .custom_formatter(|v, _| format!("{:02}:{:02}", v as u16 / 60, v as u16 % 60)),
            )
            .changed();
        ui.end_row();
        ui.label(tr("Daylight:"));
        ui.label(initial.map_or_else(
            || "—".into(),
            |(a, b)| format!("{:02}:{:02} – {:02}:{:02}", a / 60, a % 60, b / 60, b % 60),
        ));
        ui.end_row();
        ui.label(tr("Darkness:"));
        changed |= ui
            .add(egui::Slider::new(&mut darkness, 0..=80).suffix("%"))
            .changed();
        ui.end_row();
        ui.label(tr("Time zone:"));
        egui::ComboBox::from_id_salt("solar_time_zone")
            .selected_text(s.utc_offset.map_or_else(
                || tr("Automatic (by longitude)"),
                |v| format!("UTC{v:+03}:00"),
            ))
            .show_ui(ui, |ui| {
                changed |= ui
                    .selectable_value(&mut s.utc_offset, None, tr("Automatic (by longitude)"))
                    .changed();
                for off in -12..=14 {
                    changed |= ui
                        .selectable_value(&mut s.utc_offset, Some(off), format!("UTC{off:+03}:00"))
                        .changed();
                }
            });
        ui.end_row();
    });
    if app.scene.geo_datum.is_some() {
        ui.label(format!(
            "{} ({lat:.3}, {lon:.3})",
            tr("Sun at the model's geolocation")
        ));
    } else {
        ui.label(tr(
            "No geolocation — using Arequipa. Add the real site for a true sun study.",
        ));
    }
    if changed {
        s.hour = (minutes / 60) as u8;
        s.minute = (minutes % 60) as u8;
        s.darkness = 1. - f64::from(darkness) / 100.;
        apply(app, s, lat, lon);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_panel_edits_publish_exact_document_settings() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../docs/audits/solar-clock/panel.json"))
                .unwrap();
        for row in fixture["rows"].as_array().unwrap() {
            let mut app = IngeTrazoApp::new();
            app.set_shadow_settings(ShadowSettings::try_from_json(&row["before"]).unwrap());
            let version = app.scene.view_version;
            apply(
                &mut app,
                ShadowSettings::try_from_json(&row["controls"]).unwrap(),
                row["lat"].as_f64().unwrap(),
                row["lon"].as_f64().unwrap(),
            );
            assert_eq!(app.scene.shadows.to_json(), row["after"], "{}", row["name"]);
            assert!(app.shadows_enabled);
            assert!(app.scene.view_version > version);
        }
        for row in fixture["day_of_year"].as_array().unwrap() {
            let d = row["day"].as_u64().unwrap() as u16;
            let (m, n) = month_day(d);
            assert_eq!(serde_json::json!([m, n]), row["month_day"]);
            assert_eq!(day_of_year(m, n), d);
        }
    }
    #[test]
    fn actual_panel_repaints_preserve_loaded_night_and_unrounded_brightness() {
        let mut app = IngeTrazoApp::new();
        let original = ShadowSettings {
            enabled: true,
            hour: 2,
            minute: 3,
            darkness: 0.537,
            utc_offset: Some(23),
            ..Default::default()
        };
        app.set_shadow_settings(original.clone());
        let version = app.scene.view_version;
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| show(&mut app, ui));
            });
        }
        assert_eq!(app.scene.shadows, original);
        assert_eq!(app.scene.view_version, version);
    }
}
