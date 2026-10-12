use ingetrazo_core::scene::SelectionItem;
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
fn record(app: &IngeTrazoApp, name: &str, phase: &str, rows: &mut Vec<Value>) {
    let p=app.survey_profile.profile.as_ref().map(|p|json!({"samples":p.samples.iter().map(|s|json!({"station":s.station,"x":s.x,"y":s.y,"elevation":s.elevation})).collect::<Vec<_>>(),"length":p.length,"complete":p.complete,"min":p.min_elevation(),"max":p.max_elevation(),"gain":p.total_gain(),"slopes":p.slopes()}));
    rows.push(json!({"case":name,"phase":phase,"profile":p,"message":app.survey_profile.message,"sampler":app.survey_profile.sampler_name(),"path":app.survey_profile.profile_path_index(&app.scene),"undo":app.undo_stack.len(),"redo":app.redo_stack.len(),"version":app.scene.version}));
}
pub fn run() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let original: Value =
        serde_json::from_slice(&std::fs::read(out.join("original-profile.json")).unwrap()).unwrap();
    let mut rows = Vec::new();
    let mut cache = Vec::new();
    let mut guards = Vec::new();
    for case in original["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut app = IngeTrazoApp::new();
        app.system_clipboard = None;
        app.set_language("en");
        app.open_document(&out.join(format!("{name}-input.igz")));
        for index in case["selected"].as_array().unwrap() {
            app.scene
                .select(SelectionItem::GeoPath(index.as_u64().unwrap() as usize));
        }
        let before = app.scene.clone();
        let undo = app.undo_stack.len();
        let redo = app.redo_stack.len();
        app.open_survey_profile();
        record(&app, name, "compute", &mut rows);
        app.open_survey_profile();
        record(&app, name, "repeat", &mut rows);
        if case["csv"].as_bool().unwrap() {
            assert!(app
                .export_survey_profile(&out.join(format!("{name}-native.csv")))
                .unwrap());
        }
        assert_eq!(app.scene, before);
        assert_eq!(app.undo_stack.len(), undo);
        assert_eq!(app.redo_stack.len(), redo);
        cache.push(json!({"case":name,"decoded":app.survey_profile.decode_count,"resident_bytes":app.survey_profile.resident_bytes()}));
        app.save_to_path(&out.join(format!("{name}-native.igz")));
        if name == "highest-gaps" {
            let accepted = app.survey_profile.profile.clone().unwrap();
            let charge = app.survey_profile.resident_bytes();
            let limits = app.survey_profile.limits;
            app.survey_profile.limits.bytes = 0;
            assert!(app.compute_survey_profile().is_err());
            assert!(std::sync::Arc::ptr_eq(
                &accepted,
                app.survey_profile.profile.as_ref().unwrap()
            ));
            assert_eq!(charge, app.survey_profile.resident_bytes());
            assert_eq!(app.scene, before);
            guards.push(json!({"guard":"budget-refusal","unchanged":true}));
            app.survey_profile.limits = limits;
            let member = app.scene.native_document["photo_mesh"]["geometry"]
                .as_str()
                .unwrap_or("survey/geometry.npz")
                .to_owned();
            let payload = app.scene.embedded_textures.remove(&member).unwrap();
            assert!(app.compute_survey_profile().is_err());
            app.scene
                .embedded_textures
                .insert(member.clone(), vec![0, 1, 2]);
            assert!(app.compute_survey_profile().is_err());
            app.scene.embedded_textures.insert(member, payload);
            assert!(std::sync::Arc::ptr_eq(
                &accepted,
                app.survey_profile.profile.as_ref().unwrap()
            ));
            assert_eq!(charge, app.survey_profile.resident_bytes());
            guards.push(json!({"guard":"missing-malformed-resource","unchanged":true}));
            let visible = app.scene.native_document["photo_mesh"]["visible"].clone();
            app.scene.native_document["photo_mesh"]["visible"] = json!(false);
            assert!(app.compute_survey_profile().unwrap_err().contains("DEM"));
            app.scene.native_document["photo_mesh"]["visible"] = visible;
            assert!(std::sync::Arc::ptr_eq(
                &accepted,
                app.survey_profile.profile.as_ref().unwrap()
            ));
            assert_eq!(app.scene, before);
            assert_eq!(app.undo_stack.len(), undo);
            assert_eq!(app.redo_stack.len(), redo);
            guards.push(json!({"guard":"hidden-photo-dem-open","unchanged":true}));
            // History publication can replace a path with the same Scene.version.
            // Its runtime owner survives, but the authoritative App epoch changes.
            let old_version = app.scene.version;
            app.save_undo_state();
            app.scene.geo_paths[0].points[1].x += 2.;
            assert!(app.compute_survey_profile().unwrap());
            let changed = app.survey_profile.profile.clone().unwrap();
            assert_ne!(changed.samples, accepted.samples);
            app.undo();
            assert_eq!(app.scene.version, old_version);
            app.refresh_survey_profile();
            assert_eq!(
                app.survey_profile.profile.as_ref().unwrap().samples,
                accepted.samples
            );
            app.redo();
            assert_eq!(app.scene.version, old_version);
            app.refresh_survey_profile();
            assert_eq!(
                app.survey_profile.profile.as_ref().unwrap().samples,
                changed.samples
            );
            assert_eq!(app.survey_profile.decode_count, 1);
            guards.push(json!({"guard":"equal-version-history-refresh","unchanged":true}));
        }
    }
    std::fs::write(out.join("native-profile.json"),serde_json::to_vec_pretty(&json!({"cases":original["cases"],"states":rows,"numpy":original["numpy"],"source_pin":original["source_pin"]})).unwrap()).unwrap();
    std::fs::write(
        out.join("profile-cache.json"),
        serde_json::to_vec_pretty(&cache).unwrap(),
    )
    .unwrap();
    std::fs::write(
        out.join("profile-guards.json"),
        serde_json::to_vec_pretty(&guards).unwrap(),
    )
    .unwrap();
}
