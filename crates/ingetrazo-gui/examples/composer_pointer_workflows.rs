//! Missing composer pointer transactions through actual App egui frames.
use ingetrazo_core::{
    composition::{Orientation, PaperSize, RectMm, Sheet, SheetItem, ViewFrame},
    scene::Scene,
};
use ingetrazo_gui::app::IngeTrazoApp;
use serde_json::{json, Value};
use std::collections::HashMap;
fn frame(ctx: &egui::Context, a: &mut IngeTrazoApp, time: f64, events: Vec<egui::Event>) {
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1100., 850.),
            )),
            time: Some(time),
            events,
            ..Default::default()
        },
        |ctx| a.update_ui(ctx),
    );
}
fn state(a: &IngeTrazoApp, names: &HashMap<uuid::Uuid, String>) -> Value {
    let sheet = &a.scene.sheets[0];
    let frames:Vec<_>=sheet.items.iter().map(|item|{let id=item.id();let r=item.rect();let m=&sheet.native_items[&id];json!({"uid":names[&id],"rect":[r.x,r.y,r.w,r.h],"locked":m["locked"],"hidden":m["hidden"],"z":m["z"]})}).collect();
    let h = a.composer_state.sheet_history.get(&sheet.id);
    json!({"frames":frames,"selected":sheet.selected_item.map(|i|vec![names[&sheet.items[i].id()].clone()]).unwrap_or_default(),"undo":h.map_or(0,|h|h.0.len()),"redo":h.map_or(0,|h|h.1.len())})
}
fn point(ctx: &egui::Context, at: &Value) -> egui::Pos2 {
    let [x, y, z] = ctx.data(|d| {
        d.get_temp::<[f32; 3]>(egui::Id::new("pdf-ui-paper-transform"))
            .unwrap()
    });
    egui::pos2(
        x + at[0].as_f64().unwrap() as f32 * z,
        y + at[1].as_f64().unwrap() as f32 * z,
    )
}
fn key(k: egui::Key, ctrl: bool) -> egui::Event {
    egui::Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            ctrl,
            command: ctrl,
            ..Default::default()
        },
    }
}
fn main() {
    std::env::set_var("INGETRAZO_PDF_UI_OBSERVE", "1");
    let args: Vec<_> = std::env::args().collect();
    let source: Value = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let mut rows = vec![];
    for row in source["cases"].as_array().unwrap() {
        let recipe = &row["recipe"];
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene = Scene::new();
        let mut sheet = Sheet::new("Pointer", PaperSize::A4, Orientation::Landscape);
        sheet.items.clear();
        sheet.margin_mm = 10.;
        let mut names = HashMap::new();
        for f in recipe["frames"].as_array().unwrap() {
            let mut frame = ViewFrame::default();
            frame.rect = RectMm::new(
                f["x_mm"].as_f64().unwrap(),
                f["y_mm"].as_f64().unwrap(),
                f["w_mm"].as_f64().unwrap(),
                f["h_mm"].as_f64().unwrap(),
            );
            frame.style = "vectorial".into();
            names.insert(frame.id, f["uid"].as_str().unwrap().to_string());
            sheet.native_items.insert(
                frame.id,
                json!({"locked":f["locked"],"hidden":f["hidden"],"z":f["z"]}),
            );
            sheet.items.push(SheetItem::ViewFrame(frame));
        }
        sheet.selected_item = sheet.items.iter().position(|i| {
            recipe["selected"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s.as_str() == Some(names[&i.id()].as_str()))
        });
        a.scene.sheets = vec![sheet];
        a.scene.active_sheet_index = Some(0);
        a.composer_state.zoom = 2.;
        let ctx = egui::Context::default();
        frame(&ctx, &mut a, 0., vec![]);
        frame(&ctx, &mut a, 0.01, vec![]);
        let mut states = vec![json!({"phase":"before","state":state(&a,&names)})];
        let mut snapshots = 0;
        let mut diagnostics = vec![];
        for (i, e) in recipe["events"].as_array().unwrap().iter().enumerate() {
            let kind = e["kind"].as_str().unwrap();
            let events = if kind == "escape" {
                vec![key(egui::Key::Escape, false)]
            } else {
                let pos = point(&ctx, &e["at"]);
                match kind {
                    "press" => vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers: Default::default(),
                        },
                    ],
                    "release" => vec![egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: Default::default(),
                    }],
                    "move" => vec![egui::Event::PointerMoved(pos)],
                    _ => panic!("unknown pointer event"),
                }
            };
            let before = a.composer_state.edit_before.is_some();
            frame(&ctx, &mut a, (i + 1) as f64 * 0.1, events);
            if !before && a.composer_state.edit_before.is_some() {
                snapshots += 1;
            }
            states.push(json!({"phase":format!("{i}-{kind}"),"state":state(&a,&names)}));
            diagnostics.push(json!({"phase":format!("{i}-{kind}"),"grab":a.composer_state.dragging_item_idx,"pending":a.composer_state.edit_before.is_some(),"snapshots":snapshots}));
        }
        let n = recipe["events"].as_array().unwrap().len();
        frame(
            &ctx,
            &mut a,
            (n + 1) as f64 * 0.1,
            vec![key(egui::Key::Z, true)],
        );
        states.push(json!({"phase":"undo","state":state(&a,&names)}));
        frame(
            &ctx,
            &mut a,
            (n + 2) as f64 * 0.1,
            vec![key(egui::Key::Y, true)],
        );
        states.push(json!({"phase":"redo","state":state(&a,&names)}));
        let idle_before = a.scene.sheets.clone();
        for j in 0..8 {
            frame(&ctx, &mut a, (n + 3 + j) as f64 * 0.1, vec![]);
        }
        let idle_unchanged =
            a.scene.sheets == idle_before && !a.composer_state.edit_before.is_some();
        let boundary_controls = if recipe["name"] == "unselected-drag" {
            let blank = point(&ctx, &json!([12., 150.]));
            frame(
                &ctx,
                &mut a,
                3.,
                vec![
                    egui::Event::PointerMoved(blank),
                    egui::Event::PointerButton {
                        pos: blank,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                ],
            );
            let blank_press_kept_selection = a.scene.sheets[0].selected_item == Some(0);
            frame(
                &ctx,
                &mut a,
                3.1,
                vec![egui::Event::PointerButton {
                    pos: blank,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }],
            );
            let blank_release_cleared_selection = a.scene.sheets[0].selected_item.is_none();
            a.scene.sheets[0].items[0].set_pos(-20., 40.);
            frame(&ctx, &mut a, 3.2, vec![]);
            let outside = point(&ctx, &json!([-10., 50.]));
            frame(
                &ctx,
                &mut a,
                3.3,
                vec![
                    egui::Event::PointerMoved(outside),
                    egui::Event::PointerButton {
                        pos: outside,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                ],
            );
            let outside_paper_acquired = a.composer_state.dragging_item_idx == Some(0)
                && a.scene.sheets[0].selected_item == Some(0);
            let moved = point(&ctx, &json!([-6., 53.]));
            frame(&ctx, &mut a, 3.4, vec![egui::Event::PointerMoved(moved)]);
            frame(
                &ctx,
                &mut a,
                3.5,
                vec![egui::Event::PointerButton {
                    pos: moved,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }],
            );
            let outside_position = a.scene.sheets[0].items[0].rect();
            let anchor = point(&ctx, &json!([14., 58.]));
            let overlay = egui::Id::new("composer-pointer-foreground");
            let mut overlay_owned = false;
            let mut overlay_details = vec![];
            let mut overlay_clicked = false;
            for (i, events) in [
                vec![],
                vec![
                    egui::Event::PointerMoved(anchor),
                    egui::Event::PointerButton {
                        pos: anchor,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                ],
                vec![egui::Event::PointerMoved(anchor + egui::vec2(4., 0.))],
                vec![egui::Event::PointerButton {
                    pos: anchor + egui::vec2(4., 0.),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }],
            ]
            .into_iter()
            .enumerate()
            {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1100., 850.),
                        )),
                        time: Some(4. + i as f64 * 0.1),
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::Area::new(overlay)
                            .order(egui::Order::Foreground)
                            .movable(false)
                            .fixed_pos(anchor - egui::vec2(20., 10.))
                            .show(ctx, |ui| {
                                let (_, response) = ui.allocate_exact_size(
                                    egui::vec2(80., 40.),
                                    egui::Sense::click(),
                                );
                                overlay_owned |= i==1 && ctx.layer_id_at(anchor)==Some(egui::LayerId::new(egui::Order::Foreground,overlay)) && ctx.input(|input|input.pointer.primary_down()) && response.contains_pointer();
                                overlay_clicked |= response.clicked();
                                overlay_details.push(json!({"step":i,"rect":[response.rect.min.x,response.rect.min.y,response.rect.max.x,response.rect.max.y],"anchor":[anchor.x,anchor.y],"hover":response.hovered(),"contains":response.contains_pointer(),"down":response.is_pointer_button_down_on()}));
                            });
                        a.update_ui(ctx);
                    },
                );
            }
            let overlay_preserved = a.scene.sheets[0].items[0].rect() == outside_position
                && a.composer_state.dragging_item_idx.is_none()
                && a.composer_state.edit_before.is_none();
            // Remove the foreground area before the next real grab.
            frame(&ctx, &mut a, 5., vec![]);
            let loss_press = point(&ctx, &json!([10., 58.]));
            frame(
                &ctx,
                &mut a,
                5.1,
                vec![
                    egui::Event::PointerMoved(loss_press),
                    egui::Event::PointerButton {
                        pos: loss_press,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                ],
            );
            let loss_move = loss_press + egui::vec2(8., 0.);
            frame(
                &ctx,
                &mut a,
                5.2,
                vec![egui::Event::PointerMoved(loss_move)],
            );
            let loss_position = a.scene.sheets[0].items[0].rect();
            let history_before_loss = a.composer_state.sheet_history[&a.scene.sheets[0].id]
                .0
                .len();
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1100., 850.),
                    )),
                    time: Some(5.3),
                    events: vec![egui::Event::PointerButton {
                        pos: loss_move,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: Default::default(),
                    }],
                    ..Default::default()
                },
                |ctx| {
                    // Another input owner consumed release, while egui still knows
                    // the hardware button is up. Exercise the capture-loss guard.
                    ctx.input_mut(|i| {
                        i.events.retain(|e| {
                            !matches!(
                                e,
                                egui::Event::PointerButton {
                                    button: egui::PointerButton::Primary,
                                    pressed: false,
                                    ..
                                }
                            )
                        })
                    });
                    a.update_ui(ctx);
                },
            );
            let lost_capture_finished = a.composer_state.dragging_item_idx.is_none()
                && a.composer_state.edit_before.is_none()
                && a.scene.sheets[0].items[0].rect() == loss_position
                && a.composer_state.sheet_history[&a.scene.sheets[0].id]
                    .0
                    .len()
                    == history_before_loss + 1;
            frame(&ctx, &mut a, 5.4, vec![key(egui::Key::Z, true)]);
            let lost_capture_undo = a.scene.sheets[0].items[0].rect() == outside_position;
            let count_before_packet = a.composer_state.sheet_history[&a.scene.sheets[0].id]
                .0
                .len();
            let p1 = point(&ctx, &json!([-6., 50.]));
            let m1 = point(&ctx, &json!([-2., 52.]));
            let p2 = point(&ctx, &json!([4., 52.]));
            let m2 = point(&ctx, &json!([10., 55.]));
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            frame(
                &ctx,
                &mut a,
                5.5,
                vec![
                    button(p1, true),
                    egui::Event::PointerMoved(m1),
                    button(m1, false),
                    button(p2, true),
                    egui::Event::PointerMoved(m2),
                    button(m2, false),
                ],
            );
            let two_packet_histories = a.composer_state.sheet_history[&a.scene.sheets[0].id]
                .0
                .len()
                == count_before_packet + 2;
            let two_packet_final = a.scene.sheets[0].items[0].rect();
            frame(&ctx, &mut a, 5.6, vec![key(egui::Key::Z, true)]);
            let two_packet_first = a.scene.sheets[0].items[0].rect();
            frame(&ctx, &mut a, 5.7, vec![key(egui::Key::Z, true)]);
            let two_packet_before = a.scene.sheets[0].items[0].rect();
            let r = a.scene.sheets[0].items[0].rect();
            json!({"blank_press_kept_selection":blank_press_kept_selection,"blank_release_cleared_selection":blank_release_cleared_selection,"outside_paper_acquired":outside_paper_acquired,"outside_paper_position":[r.x,r.y],"foreground_layer_owned_press":overlay_owned,"foreground_preserved_item":overlay_preserved,"lost_capture_finished":lost_capture_finished,"lost_capture_undo":lost_capture_undo,"two_packet_histories":two_packet_histories,"two_packet_final":[two_packet_final.x,two_packet_final.y],"two_packet_first":[two_packet_first.x,two_packet_first.y],"two_packet_before":[two_packet_before.x,two_packet_before.y]})
        } else {
            Value::Null
        };
        rows.push(json!({"name":recipe["name"],"states":states,"snapshots":snapshots,"diagnostics":diagnostics,"idle_unchanged":idle_unchanged,"boundary_controls":boundary_controls}));
    }
    std::fs::write(
        &args[2],
        serde_json::to_vec_pretty(&json!({"cases":rows})).unwrap(),
    )
    .unwrap();
}
