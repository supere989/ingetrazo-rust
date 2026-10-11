use glam::{DVec2, DVec3};
use ingetrazo_core::scene::SelectionItem;
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::base::{AnnotationEditTarget, ToolContext, ToolType};
use serde_json::json;
fn target(a: &IngeTrazoApp) -> Option<serde_json::Value> {
    a.active_tool
        .annotation_hover_target(&a.scene)
        .and_then(|t| match t {
            AnnotationEditTarget::Label(id) => a
                .scene
                .text_labels
                .iter()
                .position(|x| x.history_id == id)
                .map(|i| json!(["label", i])),
            AnnotationEditTarget::Dimension(id) => a
                .scene
                .dimensions
                .iter()
                .position(|x| x.id == id)
                .map(|i| json!(["dimension", i])),
        })
}
fn inks(a: &IngeTrazoApp) -> serde_json::Value {
    use ingetrazo_render::annotations::Primitive;
    let plan = ingetrazo_gui::annotation_plan_for_probe(
        a,
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.)),
    )
    .unwrap();
    let mut labels = std::collections::BTreeSet::new();
    let mut dimensions = std::collections::BTreeSet::new();
    for op in &plan.operations {
        match &op.primitive {
            Primitive::Text { text, color, .. }
                if a.scene.text_labels[0]
                    .text
                    .lines()
                    .any(|line| line == text.as_ref())
                    && *color != [255, 255, 255, 230] =>
            {
                labels.insert(*color);
            }
            Primitive::Line { width, color, .. } if *width == 1.5 => {
                dimensions.insert(*color);
            }
            _ => {}
        }
    }
    // This fixture has one dimension and one two-line label: every actual
    // foreground operation for each target must carry the same opaque ink.
    let extract = |set: std::collections::BTreeSet<[u8; 4]>| {
        assert!(set.len() <= 1, "inconsistent target foreground inks");
        set.first().map(|c| {
            assert_eq!(c[3], 255);
            [c[0], c[1], c[2]]
        })
    };
    json!({"label":extract(labels),"dimension":extract(dimensions)})
}
fn movement(a: &mut IngeTrazoApp, p: DVec2) {
    a.update_move_annotation_hits([800., 600.], p).unwrap();
    let vp = ingetrazo_core::screen_pick::PixelViewport::new(&a.camera, 800., 600.);
    let eye = a.camera.eye();
    a.active_tool.note_pick_viewport(vp, eye, a.model_revision);
    let mut ctx = ToolContext::new(DVec3::ZERO, a.camera.screen_to_ray(p.x, p.y, 800., 600.));
    ctx.screen = Some(p);
    a.dispatch_tool_move(&ctx);
}
fn main() {
    let p = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let original: serde_json::Value =
        serde_json::from_slice(&std::fs::read(p.join("original-hover.json")).unwrap()).unwrap();
    let mut rows = vec![];
    for case in [
        "glyph",
        "dimension",
        "leader",
        "edge-priority",
        "group-priority",
        "hidden",
        "locked",
        "nested",
    ] {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene = ingetrazo_formats::igz::load_igz(&p.join("input.igz")).unwrap();
        if case == "nested" {
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
        }
        a.camera.target = DVec3::ZERO;
        a.camera.distance = 10.;
        a.camera.yaw = 0.;
        a.camera.pitch = std::f64::consts::FRAC_PI_2;
        a.camera.perspective = false;
        a.camera.aspect = 800. / 600.;
        let layer = a
            .scene
            .layers
            .iter_mut()
            .find(|x| x.name == "named")
            .unwrap();
        layer.visible = case != "hidden";
        layer.locked = case == "locked";
        if case == "edge-priority" {
            let a_id = a.scene.mesh.add_vertex(DVec3::new(-1., 1., 0.));
            let b_id = a.scene.mesh.add_vertex(DVec3::new(1., 1., 0.));
            a.scene.mesh.add_edge(a_id, b_id);
        }
        if case == "group-priority" {
            let mut m = ingetrazo_core::mesh::Mesh::new();
            let ids = [
                [-1., 0.5, 0.1],
                [1., 0.5, 0.1],
                [1., 1.5, 0.1],
                [-1., 1.5, 0.1],
            ]
            .map(|p| m.add_vertex(DVec3::from_array(p)));
            m.add_face(ids.to_vec(), vec![]);
            let mut group = ingetrazo_core::groups::Group::new("hover-occluder");
            group.mesh = m;
            a.scene.groups.push(group);
        }
        a.scene.clear_selection();
        if case == "glyph" || case == "dimension" {
            a.scene.native_document["dimension_style"]["color"] = json!([11, 22, 33]);
            a.scene.select(if case == "glyph" {
                SelectionItem::Dimension(0)
            } else {
                SelectionItem::TextLabel(0)
            });
        }
        a.set_tool(ToolType::Select);
        let old = a.scene.clone();
        let revision = a.model_revision;
        let dirty=a.document_dirty();
        let geometry = a.native_geometry_revision;
        let undo = a.undo_stack.len();
        let redo = a.redo_stack.len();
        let r = original["states"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["case"] == case)
            .unwrap();
        let screen = DVec2::new(
            r["screen"][0].as_f64().unwrap(),
            r["screen"][1].as_f64().unwrap(),
        );
        let record = |a: &IngeTrazoApp, phase: &str| json!({"case":case,"phase":phase,"screen":screen.to_array(),"annotation_hover":target(a),"inks":inks(a),"selection":a.scene.selection.len(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"depth":a.scene.history_context.len(),"same_identity":true});
        rows.push(record(&a, "before"));
        movement(&mut a, screen);
        rows.push(record(&a, "hover"));
        let held = a.active_tool.annotation_hover_target(&a.scene);
        movement(&mut a, screen);
        assert_eq!(a.active_tool.annotation_hover_target(&a.scene), held);
        rows.push(record(&a, "same"));
        movement(&mut a, DVec2::new(790., 590.));
        rows.push(record(&a, "empty"));
        movement(&mut a, screen);
        a.set_tool(ToolType::Line);
        rows.push(record(&a, "retire"));
        assert_eq!(a.scene, old);
        assert_eq!(a.document_dirty(),dirty);
        assert_eq!(
            (
                a.model_revision,
                a.native_geometry_revision,
                a.undo_stack.len(),
                a.redo_stack.len()
            ),
            (revision, geometry, undo, redo)
        );
        if case == "glyph" || case == "dimension" {
            a.set_tool(ToolType::Select);
            let view = a.scene.view_version;
            let empty = DVec2::new(790., 590.);
            movement(&mut a, empty);
            let mut ctx = ToolContext::new(
                DVec3::ZERO,
                a.camera.screen_to_ray(empty.x, empty.y, 800., 600.),
            );
            ctx.screen = Some(empty);
            a.dispatch_tool_click(&ctx);
            assert!(a.scene.view_version > view);
            rows.push(record(&a, "clear-selection"));
            assert_eq!((a.undo_stack.len(), a.redo_stack.len()), (undo, redo));
        }
    }
    std::fs::write(
        p.join("native-hover.json"),
        serde_json::to_vec_pretty(&json!({"states":rows})).unwrap(),
    )
    .unwrap();
}
