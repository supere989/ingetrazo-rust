//! UI appearance is session state; it does not mutate the drawing or history.
use crate::session_state::Preferences;
use egui::{Color32, Theme, ThemePreference};

pub fn theme(p: &Preferences) -> &'static str {
    match p.extra.get("general/theme").and_then(|v| v.as_str()) {
        Some("light") => "light",
        Some("system") => "system",
        _ => "dark",
    }
}
pub fn toolbar_size(p: &Preferences) -> u32 {
    let raw = p.extra.get("ui/toolbar_icon_px");
    let n = match raw {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(s)) if s.is_empty() => None,
        Some(serde_json::Value::String(s)) => Some(s.trim().parse::<i64>().unwrap_or(24)),
        Some(v) => Some(
            v.as_i64()
                .or_else(|| v.as_f64().filter(|n| n.is_finite()).map(|n| n as i64))
                .unwrap_or(24),
        ),
    }
    .unwrap_or_else(|| {
        if p.extra
            .get("ui/large_toolbar_icons")
            .is_some_and(|v| v.as_str() == Some("1") || v.as_i64() == Some(1))
        {
            32
        } else {
            24
        }
    });
    match n {
        20 | 24 | 32 | 40 => n as u32,
        _ => 24,
    }
}
/// Source only persists these choices when their effective value changed.
pub fn prepare_accept(before: &Preferences, after: &mut Preferences) {
    for (key, same) in [
        ("general/theme", theme(before) == theme(after)),
        (
            "ui/toolbar_icon_px",
            toolbar_size(before) == toolbar_size(after),
        ),
    ] {
        if same {
            if let Some(v) = before.extra.get(key) {
                after.extra.insert(key.into(), v.clone());
            } else {
                after.extra.remove(key);
            }
        } else if key == "ui/toolbar_icon_px" {
            after.extra.remove("ui/large_toolbar_icons");
        }
    }
}
pub fn render(p: &mut Preferences, ui: &mut egui::Ui) {
    let old = theme(p);
    let mut chosen = old;
    egui::ComboBox::from_id_salt("pref_theme")
        .selected_text(label(old))
        .show_ui(ui, |ui| {
            for code in ["dark", "light", "system"] {
                ui.selectable_value(&mut chosen, code, label(code));
            }
        });
    if chosen != old {
        p.extra
            .insert("general/theme".into(), serde_json::json!(chosen));
    }
    let old = toolbar_size(p);
    let mut chosen = old;
    egui::ComboBox::from_id_salt("pref_icon_size")
        .selected_text(format!("Toolbar icons: {old} px"))
        .show_ui(ui, |ui| {
            for (px, label) in [
                (20, "Small"),
                (24, "Normal"),
                (32, "Large"),
                (40, "Extra large"),
            ] {
                ui.selectable_value(&mut chosen, px, format!("{label} ({px} px)"));
            }
        });
    if chosen != old {
        p.extra
            .insert("ui/toolbar_icon_px".into(), serde_json::json!(chosen));
    }
}
fn label(code: &str) -> String {
    ingetrazo_core::i18n::tr(match code {
        "light" => "Light",
        "system" => "Same as the system",
        _ => "Dark",
    })
}

fn visuals(dark: bool) -> egui::Visuals {
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    let rgb = Color32::from_rgb;
    let window = if dark {
        rgb(45, 45, 48)
    } else {
        rgb(239, 239, 239)
    };
    let text = if dark {
        rgb(224, 224, 224)
    } else {
        rgb(20, 22, 26)
    };
    v.window_fill = window;
    v.panel_fill = window;
    v.extreme_bg_color = if dark {
        rgb(37, 37, 40)
    } else {
        Color32::WHITE
    };
    v.faint_bg_color = if dark { window } else { rgb(247, 247, 247) };
    v.override_text_color = Some(text);
    v.hyperlink_color = if dark {
        rgb(74, 163, 224)
    } else {
        rgb(26, 110, 190)
    };
    v.selection.bg_fill = rgb(42, 93, 143);
    v.selection.stroke.color = Color32::WHITE;
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.fg_stroke.color = text;
    }
    v.widgets.noninteractive.bg_fill = window;
    v.widgets.inactive.bg_fill = window;
    v.widgets.inactive.weak_bg_fill = window;
    v
}
/// Configure each context once. Warm frames only compare the small preference;
/// egui-winit supplies OS theme changes through RawInput on both native backends.
pub fn apply(p: &Preferences, ctx: &egui::Context) {
    let initialized = ctx.data_mut(|d| {
        let id = egui::Id::new("ingetrazo-ui-palettes");
        if d.get_temp::<bool>(id) == Some(true) {
            true
        } else {
            d.insert_temp(id, true);
            false
        }
    });
    if !initialized {
        ctx.style_mut_of(Theme::Dark, |s| s.visuals = visuals(true));
        ctx.style_mut_of(Theme::Light, |s| s.visuals = visuals(false));
    }
    let preference = match theme(p) {
        "light" => ThemePreference::Light,
        "system" => ThemePreference::System,
        _ => ThemePreference::Dark,
    };
    if ctx.options(|o| o.theme_preference != preference || o.fallback_theme != Theme::Dark) {
        ctx.options_mut(|o| {
            o.theme_preference = preference;
            o.fallback_theme = Theme::Dark;
        });
    }
    ctx.data_mut(|d| {
        let id = egui::Id::new("ingetrazo-toolbar-icon-px");
        let px = toolbar_size(p);
        if d.get_temp::<u32>(id) != Some(px) {
            d.insert_temp(id, px);
        }
    });
}
pub fn current_toolbar_size(ctx: &egui::Context) -> f32 {
    ctx.data(|d| d.get_temp::<u32>(egui::Id::new("ingetrazo-toolbar-icon-px")))
        .unwrap_or(24) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::IngeTrazoApp;
    fn frame(app: &mut IngeTrazoApp, ctx: &egui::Context, time: &mut f64) -> egui::FullOutput {
        *time += 0.1;
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1500., 1000.),
                )),
                time: Some(*time),
                ..Default::default()
            },
            |ctx| app.update_ui(ctx),
        )
    }
    fn text(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
        output
            .shapes
            .iter()
            .find_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    (t.galley.job.text == label).then_some(t.pos + t.galley.rect.center().to_vec2())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| panic!("missing appearance control {label}"))
    }
    fn click(app: &mut IngeTrazoApp, ctx: &egui::Context, time: &mut f64, pos: egui::Pos2) {
        for pressed in [true, false] {
            *time += 0.1;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1500., 1000.),
                    )),
                    time: Some(*time),
                    events: vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                    ..Default::default()
                },
                |ctx| app.update_ui(ctx),
            );
        }
    }
    fn choose(
        app: &mut IngeTrazoApp,
        ctx: &egui::Context,
        time: &mut f64,
        combo: &str,
        item: &str,
    ) {
        let output = frame(app, ctx, time);
        click(app, ctx, time, text(&output, combo));
        let output = frame(app, ctx, time);
        click(app, ctx, time, text(&output, item));
    }
    #[test]
    fn actual_appearance_choices_cancel_restart_and_composer_cache() {
        let root = std::env::temp_dir().join(format!("appearance-ui-{}", uuid::Uuid::new_v4()));
        let mut app = IngeTrazoApp::new();
        app.set_language("en");
        app.attach_session_store(crate::session_state::SessionStore::isolated(&root));
        app.persist_preferences();
        let before = app.scene.clone();
        let depth = app.undo_stack.len();
        let settings = std::fs::read(root.join("config/settings.json")).unwrap();
        let ctx = egui::Context::default();
        let mut time = 0.;
        frame(&mut app, &ctx, &mut time);
        app.begin_preferences();
        frame(&mut app, &ctx, &mut time);
        choose(&mut app, &ctx, &mut time, "Dark", "Light");
        choose(
            &mut app,
            &ctx,
            &mut time,
            "Toolbar icons: 24 px",
            "Extra large (40 px)",
        );
        assert_eq!(theme(&app.document_session.preferences), "dark");
        assert_eq!(toolbar_size(&app.document_session.preferences), 24);
        let output = frame(&mut app, &ctx, &mut time);
        click(&mut app, &ctx, &mut time, text(&output, "Cancel"));
        assert_eq!(
            std::fs::read(root.join("config/settings.json")).unwrap(),
            settings
        );
        assert_eq!(app.scene, before);
        app.begin_preferences();
        frame(&mut app, &ctx, &mut time);
        choose(&mut app, &ctx, &mut time, "Dark", "Light");
        choose(
            &mut app,
            &ctx,
            &mut time,
            "Toolbar icons: 24 px",
            "Extra large (40 px)",
        );
        let output = frame(&mut app, &ctx, &mut time);
        click(&mut app, &ctx, &mut time, text(&output, "OK"));
        frame(&mut app, &ctx, &mut time);
        assert!(!ctx.style().visuals.dark_mode);
        assert_eq!(current_toolbar_size(&ctx), 40.);
        assert_eq!(app.scene, before);
        assert_eq!(app.undo_stack.len(), depth);
        let mut restarted = IngeTrazoApp::new();
        restarted.attach_session_store(crate::session_state::SessionStore::isolated(&root));
        assert_eq!(theme(&restarted.document_session.preferences), "light");
        assert_eq!(toolbar_size(&restarted.document_session.preferences), 40);
        app.scene.sheets = vec![ingetrazo_core::composition::Sheet::default()];
        app.scene.active_sheet_index = Some(0);
        app.enter_composer_sheet(0);
        frame(&mut app, &ctx, &mut time);
        frame(&mut app, &ctx, &mut time);
        assert_eq!(app.composer_state.tool_switch_rects[0].height(), 44.);
        let cached = app.icons.cached_icons();
        let texture = app.icons.get(&ctx, "text").unwrap().id();
        frame(&mut app, &ctx, &mut time);
        assert_eq!(app.icons.cached_icons(), cached);
        assert_eq!(app.icons.get(&ctx, "text").unwrap().id(), texture);
        app.begin_preferences();
        frame(&mut app, &ctx, &mut time);
        choose(
            &mut app,
            &ctx,
            &mut time,
            "Toolbar icons: 40 px",
            "Small (20 px)",
        );
        let output = frame(&mut app, &ctx, &mut time);
        click(&mut app, &ctx, &mut time, text(&output, "OK"));
        frame(&mut app, &ctx, &mut time);
        assert_eq!(app.composer_state.tool_switch_rects[0].height(), 24.);
        std::fs::remove_dir_all(root).unwrap();
    }
}
