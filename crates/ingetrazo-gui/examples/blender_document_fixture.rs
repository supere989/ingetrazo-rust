use ingetrazo_core::render_blender::{Ambience, BlenderDocument, LightKind};
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../docs/audits/blender-document-follow/cases.json"
    ))
    .unwrap();
    let clean = cases
        .iter()
        .map(|v| BlenderDocument::from_json(v).to_json())
        .collect::<Vec<_>>();
    let mut app = IngeTrazoApp::new();
    app.system_clipboard = None;
    app.sync_blender_document();
    let mut rows = Vec::new();
    fn snap(app: &mut IngeTrazoApp, rows: &mut Vec<Value>, name: &str) {
        app.sync_blender_document();
        rows.push(json!({"name":name,"document":app.blender_document_controls().to_json()}));
    }
    snap(&mut app, &mut rows, "initial");
    let revision = app.native_geometry_revision;
    app.add_blender_light_at(LightKind::Point, glam::DVec3::new(1., 2., 3.), true)
        .unwrap();
    snap(&mut app, &mut rows, "point");
    app.add_blender_light_at(LightKind::Spot, glam::DVec3::new(4., 5., 6.), false)
        .unwrap();
    snap(&mut app, &mut rows, "spot");
    assert_eq!(app.native_geometry_revision, revision);
    let mut d = app.blender_document_controls();
    d.ambience = Ambience::Night;
    d.sun_scale = 1.5;
    d.lights[1].dir = [2., -1., -3.];
    app.set_blender_document(d.to_json()).unwrap();
    snap(&mut app, &mut rows, "edited");
    app.undo();
    snap(&mut app, &mut rows, "undo");
    app.redo();
    snap(&mut app, &mut rows, "redo");
    let mut d = app.blender_document_controls();
    d.lights.remove(0);
    app.set_blender_document(d.to_json()).unwrap();
    snap(&mut app, &mut rows, "delete");
    app.undo();
    snap(&mut app, &mut rows, "undo-delete");
    ingetrazo_formats::igz::save_igz(&app.scene, &out).unwrap();
    let reopened = ingetrazo_formats::igz::load_igz(&out).unwrap();
    let reopen = BlenderDocument::from_json(&reopened.plugin_data["render_blender"]).to_json();
    let cameras = [[0.1,0.2,0.3],[123.456,-12.345,0.125]].map(|target| { let mut cam=ingetrazo_core::camera::OrbitCamera::new(); cam.target=glam::DVec3::from_array(target); ingetrazo_core::render_blender::validate_camera(&cam).unwrap(); json!({"camera":ingetrazo_core::render_blender::camera_dict(&cam),"key":serde_json::from_str::<Value>(&ingetrazo_gui::blender::follow_camera_key(&cam)).unwrap()}) });
    println!(
        "{}",
        json!({"clean":clean,"states":rows,"reopen":reopen,"cameras":cameras})
    );
}
