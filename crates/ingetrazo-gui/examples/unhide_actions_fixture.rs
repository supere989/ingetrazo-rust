use glam::DVec3 as V;
use ingetrazo_core::{
    groups::Group, saved_views::VisibilityTarget as T, scene::SelectionItem as S,
};
use ingetrazo_gui::IngeTrazoApp as App;
use serde_json::{json, Value};
fn setup() -> (App, Vec<T>) {
    let mut a = App::new();
    for (p, q) in [(V::ZERO, V::X), (V::new(2., 0., 0.), V::new(3., 0., 0.))] {
        let p = a.scene.mesh.add_vertex(p);
        let q = a.scene.mesh.add_vertex(q);
        a.scene.mesh.add_edge(p, q).unwrap();
    }
    let ids = [V::new(0., 2., 0.), V::new(1., 2., 0.), V::new(0., 3., 0.)]
        .map(|v| a.scene.mesh.add_vertex(v));
    a.scene.mesh.add_face(ids.to_vec(), vec![]).unwrap();
    let mut g = Group::new("parent");
    g.children.push(Group::new("child"));
    a.scene.groups.push(g);
    let held = vec![
        T::Edge(a.scene.mesh.edges[0].history_id),
        T::Edge(a.scene.mesh.edges[1].history_id),
        T::Face(a.scene.mesh.faces[0].history_id),
        T::Group(a.scene.groups[0].history_id),
        T::Group(a.scene.groups[0].children[0].history_id),
    ];
    (a, held)
}
fn select(a: &mut App, items: &[S]) {
    a.scene.clear_selection();
    for item in items {
        a.scene.select(item.clone());
    }
}
fn live(a: &App, t: T) -> bool {
    match t {
        T::Edge(id) => a.scene.mesh.edges.iter().any(|e| e.history_id == id),
        T::Face(id) => a.scene.mesh.faces.iter().any(|f| f.history_id == id),
        T::Group(id) => a
            .scene
            .groups
            .iter()
            .any(|g| g.history_id == id || g.children.iter().any(|c| c.history_id == id)),
    }
}
fn rec(a: &App, label: &str, held: &[T], out: &mut Vec<Value>) {
    let selected = a
        .scene
        .selection
        .iter()
        .filter_map(|s| match s {
            S::Edge(i) => Some(T::Edge(a.scene.mesh.edges[*i].history_id)),
            S::Face(i) => Some(T::Face(a.scene.mesh.faces[*i].history_id)),
            S::Group(i) => Some(T::Group(a.scene.groups[*i].history_id)),
            S::ContextGroup { group, .. } => a
                .scene
                .groups
                .iter()
                .flat_map(|g| &g.children)
                .find(|c| c.id == *group)
                .map(|c| T::Group(c.history_id)),
            _ => None,
        })
        .filter_map(|t| held.iter().position(|h| *h == t))
        .collect::<Vec<_>>();
    out.push(json!({"label":label,"held_hidden":held.iter().map(|t|a.visibility_target_hidden(*t).unwrap_or(false)).collect::<Vec<_>>(),"held_live":held.iter().map(|t|live(a,*t)).collect::<Vec<_>>(),"edge_hidden":a.scene.mesh.edges.iter().map(|e|e.hidden).collect::<Vec<_>>(),"face_hidden":a.scene.mesh.faces.iter().map(|f|f.attrs.get("hidden")==Some(&true.into())).collect::<Vec<_>>(),"group_hidden":a.scene.groups.iter().map(|g|json!([g.hidden,g.children.iter().map(|c|c.hidden).collect::<Vec<_>>()])).collect::<Vec<_>>(),"selected":selected,"undo":a.undo_stack.len(),"redo":a.redo_stack.len()}));
}
fn main() {
    let mut out = vec![];
    let (mut a, h) = setup();
    a.scene.mesh.edges[0].hidden = true;
    select(&mut a, &[S::Edge(0), S::Edge(1), S::Face(0), S::Group(0)]);
    a.hide_selection();
    rec(&a, "hide-filter", &h, &mut out);
    a.unhide_last();
    rec(&a, "last-omits-face", &h, &mut out);
    let child = a.scene.groups[0].children[0].id;
    select(
        &mut a,
        &[
            S::Edge(0),
            S::Face(0),
            S::ContextGroup {
                context: vec![],
                group: child,
            },
        ],
    );
    a.unhide_selected();
    rec(&a, "selected-hidden-only", &h, &mut out);
    a.unhide_selected();
    rec(&a, "selected-noop", &h, &mut out);
    a.scene.groups[0].children[0].hidden = true;
    a.unhide_all();
    rec(&a, "all-nested", &h, &mut out);
    a.unhide_all();
    rec(&a, "all-noop", &h, &mut out);
    a.undo();
    rec(&a, "all-undo", &h, &mut out);
    a.redo();
    rec(&a, "all-redo", &h, &mut out);
    let (mut a, h) = setup();
    select(&mut a, &[S::Face(0)]);
    a.hide_selection();
    rec(&a, "face-hide", &h, &mut out);
    a.unhide_last();
    rec(&a, "face-last-noop", &h, &mut out);
    let (mut a, h) = setup();
    select(&mut a, &[S::Edge(1)]);
    a.hide_selection();
    select(&mut a, &[S::Edge(0)]);
    a.delete_selection();
    a.unhide_last();
    rec(&a, "compacted-edge-last", &h, &mut out);
    a.undo();
    rec(&a, "compacted-unhide-undo", &h, &mut out);
    a.undo();
    rec(&a, "compacted-delete-undo", &h, &mut out);
    let (mut a, h) = setup();
    select(&mut a, &[S::Edge(1)]);
    a.hide_selection();
    select(&mut a, &[S::Edge(1)]);
    a.delete_selection();
    a.unhide_last();
    rec(&a, "detached-edge-last", &h, &mut out);
    a.unhide_last();
    rec(&a, "detached-edge-last-noop", &h, &mut out);
    a.undo();
    rec(&a, "detached-last-undo", &h, &mut out);
    a.undo();
    rec(&a, "detached-delete-undo", &h, &mut out);
    let (mut a, h) = setup();
    select(&mut a, &[S::Group(0)]);
    a.hide_selection();
    select(&mut a, &[S::Group(0)]);
    a.delete_selection();
    a.unhide_last();
    rec(&a, "detached-group-last", &h, &mut out);
    a.unhide_last();
    rec(&a, "detached-group-last-noop", &h, &mut out);
    a.undo();
    a.undo();
    rec(&a, "detached-group-restored", &h, &mut out);
    let (mut a, h) = setup();
    select(&mut a, &[S::Edge(1)]);
    a.hide_selection();
    select(&mut a, &[S::Edge(1)]);
    a.delete_selection();
    a.add_edge_command(V::new(2., 0., 0.), V::new(3., 0., 0.), None, None)
        .unwrap();
    a.unhide_last();
    rec(&a, "recreated-edge-distinct", &h, &mut out);
    let (mut a, h) = setup();
    a.save_undo_state();
    let p = a.scene.mesh.add_vertex(V::new(5., 0., 0.));
    let q = a.scene.mesh.add_vertex(V::new(6., 0., 0.));
    a.scene.mesh.add_edge(p, q);
    a.scene.bump_version();
    a.seal_snapshot_visibility();
    a.scene.mesh.edges[1].hidden = true;
    a.scene.mesh.faces[0]
        .attrs
        .insert("hidden".into(), true.into());
    a.undo();
    rec(&a, "snapshot-rewinds-flags", &h, &mut out);
    a.redo();
    rec(&a, "snapshot-redo-flags", &h, &mut out);
    let (mut a, h) = setup();
    a.add_edge_command(V::new(5., 0., 0.), V::new(6., 0., 0.), None, None)
        .unwrap();
    a.scene.mesh.edges[1].hidden = true;
    a.scene.mesh.faces[0]
        .attrs
        .insert("hidden".into(), true.into());
    a.undo();
    rec(&a, "delta-keeps-flags", &h, &mut out);
    std::fs::write(
        std::env::args().nth(1).unwrap(),
        serde_json::to_vec_pretty(&out).unwrap(),
    )
    .unwrap();
    let (mut a, mut held) = setup();
    for (g, z) in [(&mut a.scene.groups[0], 5.)] {
        let ids =
            [V::new(0., 0., z), V::new(1., 0., z), V::new(0., 1., z)].map(|v| g.mesh.add_vertex(v));
        g.mesh.add_face(ids.to_vec(), vec![]);
    }
    {
        let g = &mut a.scene.groups[0].children[0];
        let ids = [V::new(0., 0., 6.), V::new(1., 0., 6.), V::new(0., 1., 6.)]
            .map(|v| g.mesh.add_vertex(v));
        g.mesh.add_face(ids.to_vec(), vec![]);
    }
    held = vec![
        held[0],
        held[2],
        held[3],
        held[4],
        T::Face(a.scene.groups[0].mesh.faces[0].history_id),
        T::Face(a.scene.groups[0].children[0].mesh.faces[0].history_id),
    ];
    a.scene.mesh.edges[0].hidden = true;
    a.scene.mesh.faces[0]
        .attrs
        .insert("hidden".into(), true.into());
    a.scene.groups[0].children[0].hidden = true;
    let mut nested = vec![];
    let nr = |a: &App, label: &str, out: &mut Vec<Value>| {
        out.push(json!({"label":label,"depth":a.component_edit_stack.len(),"flags":held.iter().map(|t|a.visibility_target_hidden(*t).unwrap_or(false)).collect::<Vec<_>>(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"selected":a.scene.selection.len()}))
    };
    select(&mut a, &[S::Group(0)]);
    a.begin_component_edit();
    a.unhide_all();
    nr(&a, "parent-all-active-only", &mut nested);
    select(&mut a, &[S::Face(0)]);
    a.hide_selection();
    nr(&a, "parent-hide-face", &mut nested);
    a.unhide_last();
    nr(&a, "parent-last-face-noop", &mut nested);
    a.undo();
    nr(&a, "parent-hide-undo", &mut nested);
    a.undo();
    nr(&a, "parent-all-undo", &mut nested);
    a.redo();
    nr(&a, "parent-all-redo", &mut nested);
    a.scene.mesh.faces[0]
        .attrs
        .insert("hidden".into(), true.into());
    a.scene.groups[0].mesh.faces[0]
        .attrs
        .insert("hidden".into(), true.into());
    select(&mut a, &[S::Group(0)]);
    a.begin_component_edit();
    a.unhide_all();
    nr(&a, "leaf-all-active-only", &mut nested);
    a.undo();
    nr(&a, "leaf-all-undo", &mut nested);
    a.redo();
    nr(&a, "leaf-all-redo", &mut nested);
    while !a.component_edit_stack.is_empty() {
        a.finish_component_edit();
    }
    nr(&a, "finished-global-flags", &mut nested);
    std::fs::write(
        std::env::args().nth(2).unwrap(),
        serde_json::to_vec_pretty(&nested).unwrap(),
    )
    .unwrap();
}
