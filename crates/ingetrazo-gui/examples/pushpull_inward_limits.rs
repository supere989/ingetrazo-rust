use glam::{DMat4, DVec3};
use ingetrazo_core::{
    groups::Group,
    math::Ray,
    mesh::Mesh,
    scene::{Scene, SelectionItem},
};
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::{
    base::{Tool, ToolContext, ToolType},
    pushpull::PushPullTool,
};
use serde_json::{json, Value};
fn point(v: &Value) -> DVec3 {
    DVec3::new(
        v[0].as_f64().unwrap(),
        v[1].as_f64().unwrap(),
        v[2].as_f64().unwrap(),
    )
}
fn load(fs: &Value) -> Mesh {
    let mut m = Mesh::new();
    for f in fs.as_array().unwrap() {
        let mut ring = |v: &Value| {
            v.as_array()
                .unwrap()
                .iter()
                .map(|p| m.add_vertex(point(p)))
                .collect::<Vec<_>>()
        };
        let outer = ring(&f["vertices"]);
        let holes = f["holes"]
            .as_array()
            .unwrap()
            .iter()
            .map(&mut ring)
            .collect();
        let id = m.add_face(outer, holes).unwrap();
        m.faces[id].attrs = serde_json::from_value(f["attrs"].clone()).unwrap();
        m.faces[id].interior = f["interior"].as_bool().unwrap();
    }
    m
}
fn snap(m: &Mesh) -> Value {
    json!(m.faces.iter().map(|f|json!({"vertices":f.vertices.iter().map(|&v|m.vertices[v].position.to_array()).collect::<Vec<_>>(),"holes":f.holes.iter().map(|h|h.iter().map(|&v|m.vertices[v].position.to_array()).collect::<Vec<_>>()).collect::<Vec<_>>(),"attrs":f.attrs,"interior":f.interior})).collect::<Vec<_>>())
}
fn main() {
    let a = std::env::args().collect::<Vec<_>>();
    let input: Value = serde_json::from_slice(&std::fs::read(&a[1]).unwrap()).unwrap();
    let mut rows = vec![];
    for case in input["cases"].as_array().unwrap() {
        let id = case["face"].as_u64().unwrap() as usize;
        let mesh = load(&case["source"]);
        let anchor = point(&case["anchor"]);
        let limit = PushPullTool::inward_limit(&mesh, id, anchor);
        let mut primitive = Scene::new();
        primitive.mesh = mesh.clone();
        let primitive_before = primitive.clone();
        let primitive_error = PushPullTool::extrude_face_with_options(
            &mut primitive,
            id,
            case["requested"].as_f64().unwrap(),
            case["ctrl"].as_bool().unwrap(),
        )
        .err();
        let primitive_atomic = primitive_error
            .as_ref()
            .map(|_| primitive == primitive_before);
        let normal = mesh.face_normal(id);
        let requested = case["requested"].as_f64().unwrap();
        let ctx = ToolContext::new(anchor, Ray::new(anchor + normal * 0.1, -normal));
        let mut scene = Scene::new();
        scene.mesh = mesh.clone();
        let mut tool = PushPullTool::new();
        let mut ctx = ctx;
        ctx.ctrl_held = case["ctrl"].as_bool().unwrap();
        tool.on_click(&mut scene, &ctx);
        assert!(tool.is_dragging, "{} failed acquisition", case["name"]);
        let mut moved = ctx.clone();
        moved.world += normal * requested;
        tool.on_move(&mut scene, &moved);
        let preview = tool.extrusion_distance;
        let preview_status = tool.status_message().map(str::to_owned);
        assert_eq!(scene.mesh, mesh);
        tool.on_cancel(&mut scene);
        assert_eq!(scene.mesh, mesh);
        assert!(tool.status_message().is_none());
        let mut app = IngeTrazoApp::new();
        app.system_clipboard = None;
        app.scene.mesh = mesh.clone();
        app.set_tool(ToolType::PushPull);
        app.dispatch_tool_click(&ctx);
        app.dispatch_tool_move(&moved);
        assert_eq!(app.scene.mesh, mesh);
        assert!(app.undo_stack.is_empty());
        app.dispatch_tool_cancel();
        assert_eq!(app.scene.mesh, mesh);
        app.dispatch_tool_click(&ctx);
        let result = app.dispatch_tool_vcb(&requested.to_string());
        let commit_status = app.status_message.clone();
        let result_mesh = app.scene.mesh.clone();
        let changed = result_mesh != mesh;
        let history = if changed {
            assert_eq!(app.undo_stack.len(), 1);
            app.undo();
            assert_eq!(app.scene.mesh, mesh);
            app.redo();
            assert_eq!(app.scene.mesh, result_mesh);
            true
        } else {
            assert!(app.undo_stack.is_empty());
            true
        };
        rows.push(json!({"name":case["name"],"limit":limit,"distance":preview,"error":result.err(),"result":snap(&result_mesh),"changed":changed,"status":commit_status,"preview_status":preview_status,"preview_cancel_exact":true,"app_history_exact":history,"primitive_error":primitive_error,"primitive_atomic_on_error":primitive_atomic}));
    }
    let mut reused = vec![];
    for c in input["reused_limit_controls"].as_array().unwrap() {
        let mesh = load(&c["source"]);
        let id = c["face"].as_u64().unwrap() as usize;
        let f = &mesh.faces[id];
        let anchor = (f
            .vertices
            .iter()
            .map(|&v| mesh.vertices[v].position)
            .sum::<DVec3>()
            / f.vertices.len() as f64)
            .as_vec3()
            .as_dvec3();
        let limit = PushPullTool::inward_limit(&mesh, id, anchor);
        assert_eq!(limit, c["limit"].as_f64());
        let tri = mesh.triangulate_face(id)[0];
        let p = (tri.0 + tri.1 + tri.2) / 3.;
        let normal = mesh.face_normal(id);
        let ctx = ToolContext::new(p, Ray::new(p + normal * 0.1, -normal));
        let mut scene = Scene::new();
        scene.mesh = mesh.clone();
        let mut tool = PushPullTool::new();
        tool.on_click(&mut scene, &ctx);
        assert!(tool.is_dragging);
        let mut moved = ctx.clone();
        moved.world -= normal * 99.;
        tool.on_move(&mut scene, &moved);
        assert_eq!(
            tool.extrusion_distance,
            c["clamped_distance"].as_f64().unwrap()
        );
        assert_eq!(scene.mesh, mesh);
        reused.push(json!({"name":c["name"],"limit":limit,"distance":tool.extrusion_distance,"preview_model_exact":true}));
    }
    let mut vcb = vec![];
    for value in [99., -99.] {
        let c = &input["cases"][0];
        let mesh = load(&c["source"]);
        let mut app = IngeTrazoApp::new();
        app.system_clipboard = None;
        app.scene.mesh = mesh.clone();
        app.set_tool(ToolType::PushPull);
        let anchor = point(&c["anchor"]);
        let id = c["face"].as_u64().unwrap() as usize;
        let n = mesh.face_normal(id);
        let ctx = ToolContext::new(anchor, Ray::new(anchor + n * 0.1, -n));
        app.dispatch_tool_click(&ctx);
        let mut moved = ctx.clone();
        moved.world -= n * 0.5;
        app.dispatch_tool_move(&moved);
        app.dispatch_tool_vcb(&value.to_string()).unwrap();
        assert!(app.active_tool.status_message().is_none());
        let result = app.scene.mesh.clone();
        let status = app.status_message.clone();
        app.undo();
        assert_eq!(app.scene.mesh, mesh);
        app.redo();
        assert_eq!(app.scene.mesh, result);
        vcb.push(
            json!({"value":value,"status":status,"result":snap(&result),"history_exact":true}),
        );
    }
    // Actual nested application context runs the same borrowed mesh in its world frame.
    let case = &input["cases"][0];
    let mesh = load(&case["source"]);
    let mut app = IngeTrazoApp::new();
    app.system_clipboard = None;
    let leaf = Group::with_mesh("leaf", mesh.clone());
    let uid = leaf.id;
    app.scene.groups.push(leaf);
    app.scene.make_component(uid).unwrap();
    let sibling = app.scene.groups[0].duplicate_instance();
    let leaf = app.scene.groups.remove(0);
    let mut parent = Group::new("parent");
    parent.transform = DMat4::from_translation(DVec3::new(4., 5., 6.));
    parent.children.push(leaf);
    app.scene.groups = vec![parent, sibling];
    app.scene.selection.insert(SelectionItem::Group(0));
    app.begin_component_edit();
    app.scene.selection.clear();
    app.scene.selection.insert(SelectionItem::Group(0));
    app.begin_component_edit();
    let before = app.scene.mesh.clone();
    let id = case["face"].as_u64().unwrap() as usize;
    let f = &before.faces[id];
    let center = f
        .vertices
        .iter()
        .map(|&v| before.vertices[v].position)
        .sum::<DVec3>()
        / f.vertices.len() as f64;
    let n = before.face_normal(id);
    let c = ToolContext::new(center, Ray::new(center + n * 0.1, -n));
    app.set_tool(ToolType::PushPull);
    app.dispatch_tool_click(&c);
    app.dispatch_tool_vcb("-99000mm").unwrap();
    let after = app.scene.mesh.clone();
    app.undo();
    assert_eq!(app.scene.mesh, before);
    app.redo();
    assert_eq!(app.scene.mesh, after);
    app.finish_component_edit();
    app.finish_component_edit();
    assert_eq!(app.undo_stack.len(), 1);
    let published = app.scene.groups.clone();
    app.undo();
    assert_eq!(app.scene.groups[1].mesh, mesh);
    app.redo();
    assert_eq!(app.scene.groups, published);
    std::fs::write(
        &a[2],
        serde_json::to_vec_pretty(&json!({"cases":rows,"nested_history_exact":true,"vcb":vcb,"reused_limit_controls":reused}))
            .unwrap(),
    )
    .unwrap();
}
