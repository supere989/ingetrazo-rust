// SPDX-License-Identifier: GPL-3.0-or-later
//! One bounded application workflow: placement, edit, ownership, refusal and IGZ.
use glam::{DMat4, DVec3};
use ingetrazo_core::{
    math::Ray,
    scene::{Scene, SelectionItem},
};
use ingetrazo_formats::{
    load_igz, save_igz,
    text3d::{self, Params},
};
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::base::{ToolContext, ToolType};
use serde_json::json;
fn main() {
    let output = std::path::PathBuf::from(std::env::args().nth(1).expect("output directory"));
    std::fs::create_dir_all(&output).unwrap();
    let mut a = IngeTrazoApp::new();
    a.system_clipboard = None;
    a.scene = Scene::new();
    let params = Params {
        text: "A O g".into(),
        font: "DejaVu Sans".into(),
        ..Default::default()
    };
    let before = a.scene.clone();
    let mut invalid = params.clone();
    invalid.height = f64::NAN;
    assert!(a.insert_text3d(&invalid).is_err());
    assert_eq!(a.scene, before);
    assert!(a.undo_stack.is_empty());
    a.insert_text3d(&params).unwrap();
    assert_eq!(a.active_tool_type, ToolType::PlaceGroup);
    let point = DVec3::new(4., 5., 0.);
    let context = ToolContext::new(point, Ray::new(point + DVec3::Z * 10., -DVec3::Z));
    a.dispatch_tool_move(&context);
    assert!(!a.active_tool.rubber_band_lines().is_empty());
    assert_eq!(a.scene, before);
    a.dispatch_tool_cancel();
    assert_eq!(a.scene, before);
    assert!(a.undo_stack.is_empty());
    a.insert_text3d(&params).unwrap();
    a.dispatch_tool_move(&context);
    a.dispatch_tool_click(&context);
    assert_eq!(a.active_tool_type, ToolType::Select);
    assert_eq!(a.scene.groups.len(), 1);
    assert_eq!(a.undo_stack.len(), 1);
    let group = a.scene.groups[0].clone();
    let id = group.id;
    assert_eq!(
        group
            .children
            .iter()
            .map(|g| g.name.as_str())
            .collect::<Vec<_>>(),
        ["A", "O", "g"]
    );
    assert!(text3d::text_is_pristine(&group));
    assert!(group.mesh.faces.is_empty());
    save_igz(&a.scene, &output.join("placed.igz")).unwrap();
    a.undo();
    assert!(a.scene.groups.is_empty());
    a.redo();
    assert_eq!(a.scene.groups[0], group);
    assert_eq!(a.scene.groups[0].history_id, group.history_id);
    let mut edited = params.clone();
    edited.text = "AV".into();
    edited.thickness = 20.;
    edited.height = 0.5;
    a.edit_text3d(id, &edited).unwrap();
    assert_eq!(a.undo_stack.len(), 2);
    assert_eq!(a.scene.groups[0].id, id);
    assert_eq!(a.scene.groups[0].transform, group.transform);
    assert_eq!(a.scene.groups[0].children.len(), 2);
    assert!(text3d::text_is_pristine(&a.scene.groups[0]));
    let changed = a.scene.groups[0].clone();
    a.undo();
    assert_eq!(a.scene.groups[0], group);
    a.redo();
    assert_eq!(a.scene.groups[0], changed);
    a.edit_text3d(id, &edited).unwrap();
    assert_eq!(a.undo_stack.len(), 2);
    assert!(a.edit_text3d(id, &invalid).is_err());
    assert_eq!(a.scene.groups[0], changed);
    assert_eq!(a.undo_stack.len(), 2);
    save_igz(&a.scene, &output.join("edited.igz")).unwrap();
    let loaded = load_igz(&output.join("edited.igz")).unwrap();
    assert!(text3d::text_is_pristine(&loaded.groups[0]));
    assert_eq!(text3d::params(&loaded.groups[0]).unwrap(), edited);
    a.scene.clear_selection();
    a.scene.select(SelectionItem::Group(0));
    a.begin_component_edit();
    assert_eq!(a.component_edit_stack.len(), 1);
    a.scene.select(SelectionItem::Group(0));
    assert_eq!(a.selected_text3d(), Some(id));
    a.open_edit_text3d();
    assert!(a.text3d_dialog.is_some());
    let context_before = a.scene.clone();
    let egui = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1280., 800.),
        )),
        events: vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        ..Default::default()
    };
    let _ = egui.run(input, |ctx| a.update_ui(ctx));
    assert!(a.text3d_dialog.is_none());
    assert_eq!(a.component_edit_stack.len(), 1);
    assert_eq!(a.scene.groups, context_before.groups);
    edited.text = "BO".into();
    edited.height = 0.75;
    edited.thickness = 0.;
    a.edit_text3d(id, &edited).unwrap();
    assert!(a.component_edit_stack.is_empty());
    assert_eq!(a.scene.groups[0].id, id);
    assert!(text3d::text_is_pristine(&a.scene.groups[0]));
    assert_eq!(
        a.scene.groups[0].children[0].transform,
        a.scene.groups[0].children[1].transform
    );
    save_igz(&a.scene, &output.join("context-flat.igz")).unwrap();
    let pristine = a.scene.groups[0].clone();
    for mutation in 0..4 {
        a.scene.groups[0] = pristine.clone();
        match mutation {
            0 => a.scene.groups[0].children[0].mesh.vertices[0].position.x += 0.25,
            1 => {
                a.scene.groups[0].children[0].mesh.faces[0]
                    .attrs
                    .insert("color".into(), json!([1., 0., 0.]));
            }
            2 => {
                a.scene.groups[0].children.pop();
            }
            _ => {
                a.scene.groups[0].children[0].transform =
                    DMat4::from_translation(DVec3::X) * a.scene.groups[0].children[0].transform
            }
        }
        let before = a.scene.groups[0].clone();
        let undo = a.undo_stack.len();
        assert!(!text3d::text_is_pristine(&before));
        assert!(a.edit_text3d(id, &params).is_err());
        assert_eq!(a.scene.groups[0], before);
        assert_eq!(a.undo_stack.len(), undo);
    }
    a.scene.groups[0] = pristine.clone();
    for child in &mut a.scene.groups[0].children {
        child.transform = DMat4::from_translation(DVec3::X) * child.transform;
    }
    assert!(text3d::text_is_pristine(&a.scene.groups[0]));
    a.scene.groups[0] = pristine;
    // A model-mutating preview never becomes the Undo base of dialog acceptance.
    a.scene.clear_selection();
    a.scene.select(SelectionItem::Group(0));
    a.set_tool(ToolType::Move);
    a.dispatch_tool_click(&context);
    let moved = ToolContext::new(point + DVec3::X * 2., context.ray);
    a.dispatch_tool_move(&moved);
    assert!(a.active_tool.placement_preview().is_some());
    let mut preview_base = a.scene.clone();
    a.active_tool.restore_transient_model(&mut preview_base);
    assert_ne!(a.scene.groups[0].transform, preview_base.groups[0].transform);
    edited.text = "AB".into();
    let undo = a.undo_stack.len();
    a.edit_text3d(id, &edited).unwrap();
    assert_eq!(a.active_tool_type, ToolType::Select);
    assert_eq!(a.undo_stack.len(), undo + 1);
    assert_eq!(
        a.scene.groups[0].transform,
        preview_base.groups[0].transform
    );
    let after = a.scene.groups[0].clone();
    a.dispatch_tool_cancel();
    assert_eq!(a.scene.groups[0], after);
    a.undo();
    assert_eq!(a.scene.groups[0], preview_base.groups[0]);
    a.redo();
    assert_eq!(a.scene.groups[0], after);
    // External/native geometry can have f64 decimal-half values even though
    // generated glyphs are already f32. The original reader must still trust it.
    let mut mesh = ingetrazo_core::mesh::Mesh::new();
    let ids = [DVec3::new(0.00005, -0.0, 0.), DVec3::X, DVec3::Z].map(|p| mesh.add_vertex(p));
    mesh.add_face(ids.to_vec(), vec![]);
    mesh.faces[0]
        .attrs
        .insert("color".into(), json!([0.1255, 0.3335, 0.0005]));
    mesh.faces[0].attrs.insert("mat".into(), json!("Ω😀"));
    let child = ingetrazo_core::groups::Group::with_mesh("Ω😀", mesh);
    let mut boundary = ingetrazo_core::groups::Group::new("boundary");
    boundary.children.push(child);
    boundary.native_document =
        json!({"component":false,"text3d":text3d::text_state(&params,&boundary.children).unwrap()});
    let mut scene = Scene::new();
    scene.groups.push(boundary);
    save_igz(&scene, &output.join("fingerprint-boundary.igz")).unwrap();
    let report = json!({"scope":"Actual application actions and egui dialog Cancel; save/reload; no physical UI or GPU pixel claim","files":["placed.igz","edited.igz","context-flat.igz"],"checks":["invalid insertion atomic","cursor preview no document change","placement cancellation","single insertion transaction","one group per nonspace glyph","insert Undo/Redo identity","20m edit in place","edit Undo/Redo identity","unchanged no history","invalid edit atomic","IGZ reload pristine","selected letter edits owner","egui modal Escape keeps context","context edit preserves common pose","flat text","four manual-edit refusals","common pose permits regeneration"]});
    std::fs::write(
        output.join("workflow.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("3D Text application workflow passed");
}
