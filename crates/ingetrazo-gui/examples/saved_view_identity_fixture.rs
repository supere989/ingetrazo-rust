use ingetrazo_core::groups::Group;
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
fn mask(gs: &[Group]) -> Vec<bool> {
    let mut out = Vec::new();
    for g in gs {
        out.push(g.hidden);
        out.extend(mask(&g.children));
    }
    out
}
fn flip(gs: &mut [Group]) {
    for g in gs {
        g.hidden = !g.hidden;
        flip(&mut g.children);
    }
}
fn restore_mask(gs: &mut [Group], mask: &[bool], next: &mut usize) {
    for g in gs {
        g.hidden = mask[*next];
        *next += 1;
        restore_mask(&mut g.children, mask, next);
    }
}
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let mut report = serde_json::Map::new();
    for name in ["literal", "legacy", "duplicate", "generated"] {
        let mut a = IngeTrazoApp::new();
        a.scene = ingetrazo_formats::igz::load_igz(&dir.join(format!("{name}.igz"))).unwrap();
        if name == "literal" {
            assert_ne!(a.scene.groups[1].id, a.scene.groups[2].id);
        }
        let existing = a.scene.saved_views.len();
        if existing > 0 {
            let before = mask(&a.scene.groups);
            flip(&mut a.scene.groups);
            a.apply_saved_view(0);
            let applied = mask(&a.scene.groups);
            assert_eq!(
                applied,
                if name == "duplicate" {
                    vec![false, true, false]
                } else {
                    before.clone()
                }
            );
            restore_mask(&mut a.scene.groups, &before, &mut 0);
        }
        let index = existing;
        a.add_saved_view("capture".into());
        let identity = a.scene.saved_views[index].history_id;
        let ids = a.scene.saved_views[index].hidden_objects.clone();
        ingetrazo_formats::igz::save_igz(
            &a.scene,
            &dir.join(format!("{name}-captured-native.igz")),
        )
        .unwrap();
        a.scene.saved_views[index].native_document =
            json!({"vendor_view":{"untouched":[1,2,3]},"hidden_objects":ids});

        flip(&mut a.scene.groups);
        a.apply_saved_view(index);
        let applied = mask(&a.scene.groups);
        flip(&mut a.scene.groups);
        a.update_selected_saved_view();
        assert_eq!(identity, a.scene.saved_views[index].history_id);
        let updated = mask(&a.scene.groups);
        a.scene.saved_views[index]
            .hidden_objects
            .as_mut()
            .unwrap()
            .push("future-placement".into());
        let capture = if name == "legacy" {
            Value::Null
        } else {
            json!(ids)
        };
        let update = if name == "legacy" {
            Value::Null
        } else {
            json!(a.scene.saved_views[index].hidden_objects)
        };
        ingetrazo_formats::igz::save_igz(&a.scene, &dir.join(format!("{name}-native.igz")))
            .unwrap();
        let mut reopened =
            ingetrazo_formats::igz::load_igz(&dir.join(format!("{name}-native.igz"))).unwrap();
        flip(&mut reopened.groups);
        let view = reopened.saved_views[index].clone();
        view.apply_scene(&mut reopened, &mut a.camera);
        assert_eq!(
            mask(&reopened.groups),
            if name == "duplicate" {
                vec![false, false, true]
            } else {
                updated.clone()
            }
        );
        a.save_undo_state();
        a.scene.mesh.add_vertex(glam::DVec3::X);
        a.scene.bump_version();
        a.rename_selected_saved_view("renamed".into());
        a.undo();
        assert_eq!(a.scene.saved_views[index].history_id, identity);
        assert_eq!(a.scene.saved_views[index].name, "renamed");
        a.redo();
        assert_eq!(a.scene.saved_views[index].history_id, identity);
        a.remove_selected_saved_view();
        a.undo();
        assert_eq!(a.scene.saved_views.len(), existing);
        a.redo();
        assert_eq!(a.scene.saved_views.len(), existing);
        report.insert(name.into(),json!({"capture_ids":capture,"apply_mask":applied,"updated_mask":updated,"updated_ids":update}));
    }
    println!("{}", Value::Object(report));
}
