use ingetrazo_core::scene::SelectionItem;
use ingetrazo_formats::dem_profile::Encoding;
use ingetrazo_gui::{dem_tiles::Source, IngeTrazoApp};
use serde_json::{json, Value};
use std::{
    path::Path,
    time::{Duration, Instant},
};
fn provider(v: &Value, base: &str) -> Source {
    Source {
        id: v["id"].as_str().unwrap().into(),
        url_template: format!("{base}{}", v["path_template"].as_str().unwrap()),
        encoding: if v["encoding"] == "mapbox" {
            Encoding::Mapbox
        } else {
            Encoding::Terrarium
        },
        max_zoom: v["max_zoom"].as_u64().unwrap() as u8,
    }
}
fn path(spec: &Value, t: &Value) -> String {
    spec["path_template"]
        .as_str()
        .unwrap()
        .replace("{z}", &t["z"].to_string())
        .replace("{x}", &t["x"].to_string())
        .replace("{y}", &t["y"].to_string())
}
fn control(base: &str, action: &str, value: &str, key: &str) {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("action", action)
        .append_pair(key, value)
        .finish();
    ureq::get(&format!("{base}/__control?{query}"))
        .call()
        .unwrap();
}
fn poll(app: &mut IngeTrazoApp, ctx: &egui::Context, predicate: impl Fn(&Value) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(18);
    loop {
        app.poll_dem_profile(ctx);
        let state = app.dem_profile_observation();
        if predicate(&state) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "DEM timeout: {state} status={} profile={:?}",
            app.status_message,
            app.survey_profile.message
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn setup(root: &Path, case: &Value, base: &str, input: &str, cache: &Path) -> IngeTrazoApp {
    let mut app = IngeTrazoApp::new();
    app.system_clipboard = None;
    app.set_language("en");
    app.open_document(&root.join(input));
    app.survey_profile.dem_cache_root = Some(cache.into());
    app.survey_profile.dem_source = provider(&case["provider"], base);
    app.survey_profile.dem_zoom = case["zoom"].as_u64().unwrap() as u8;
    app.survey_profile.dem_deadline = Duration::from_secs(15);
    for i in case["selected"].as_array().unwrap() {
        app.scene
            .select(SelectionItem::GeoPath(i.as_u64().unwrap() as usize));
    }
    app
}
fn seed(root: &Path, case: &Value, cache: &Path) {
    let spec = &case["provider"];
    for t in case["cache_seed"].as_array().into_iter().flatten() {
        let dest = cache
            .join(spec["id"].as_str().unwrap())
            .join(t["z"].to_string())
            .join(t["x"].to_string())
            .join(t["y"].to_string());
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::copy(root.join(t["file"].as_str().unwrap()), dest).unwrap();
    }
    if case["corrupt_cache_seed"].is_object() {
        let t = &case["corrupt_cache_seed"];
        let dest = cache
            .join(spec["id"].as_str().unwrap())
            .join(t["z"].to_string())
            .join(t["x"].to_string())
            .join(t["y"].to_string());
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(dest, b"not a PNG").unwrap();
    }
}
fn record(app: &IngeTrazoApp, name: &str, phase: &str, csv: &Value, rows: &mut Vec<Value>) {
    let p=app.survey_profile.profile.as_ref().map(|p|json!({"samples":p.samples.iter().map(|s|json!({"station":s.station,"x":s.x,"y":s.y,"elevation":s.elevation})).collect::<Vec<_>>(),"length":p.length,"complete":p.complete,"min":p.min_elevation(),"max":p.max_elevation(),"gain":p.total_gain(),"slopes":p.slopes()}));
    let obs = app.dem_profile_observation();
    rows.push(json!({"case":name,"phase":phase,"profile":p,"message":app.survey_profile.message,"sampler":app.survey_profile.sampler_name(),"path":app.survey_profile.profile_path_index(&app.scene),"undo":app.undo_stack.len(),"redo":app.redo_stack.len(),"version":app.scene.version,"provider_id":obs["provider_id"],"arrived":obs["arrived"],"csv":csv}));
}
pub fn run() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let source: Value =
        serde_json::from_slice(&std::fs::read(root.join("original-dem.json")).unwrap()).unwrap();
    let cfg: Value =
        serde_json::from_slice(&std::fs::read(root.join("runtime-provider.json")).unwrap())
            .unwrap();
    let base = cfg["base_url"].as_str().unwrap();
    let ctx = egui::Context::default();
    let mut rows = vec![];
    let mut guards = vec![];
    for case in source["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let cache = root.join("native-disk").join(name);
        seed(&root, case, &cache);
        let mut app = setup(&root, case, base, case["input"].as_str().unwrap(), &cache);
        let mut before = app.scene.clone();
        let mut undo = app.undo_stack.len();
        let mut redo = app.redo_stack.len();
        let states = source["states"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["case"] == name)
            .collect::<Vec<_>>();
        for expected in states {
            let phase = expected["phase"].as_str().unwrap();
            match phase {
                "initial" => {
                    assert!(app.compute_survey_profile().unwrap());
                }
                "first-arrival" => {
                    let t = &case["tiles"][0];
                    control(base, "release", &path(&case["provider"], t), "path");
                    poll(&mut app, &ctx, |o| {
                        o["arrived"].as_array().unwrap().len() == 1
                    });
                }
                "all-responses" => {
                    control(base, "release-all", &format!("/{name}/"), "prefix");
                    let n = case["response_status"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|v| **v == 200)
                        .count();
                    poll(&mut app, &ctx, |o| {
                        o["arrived"].as_array().unwrap().len() == n && o["pending"] == 0
                    });
                }
                "new-provider-gaps" => {
                    app.survey_profile.dem_source = provider(&case["provider_variant"], base);
                    assert!(app.compute_survey_profile().unwrap());
                }
                "old-provider-arrived" => {
                    control(
                        base,
                        "release",
                        &path(&case["provider"], &case["tiles"][0]),
                        "path",
                    );
                    let tile = &case["tiles"][0];
                    let dest = cache
                        .join(case["provider"]["id"].as_str().unwrap())
                        .join(tile["z"].to_string())
                        .join(tile["x"].to_string())
                        .join(tile["y"].to_string());
                    let deadline = Instant::now() + Duration::from_secs(18);
                    while !dest.exists() {
                        app.poll_dem_profile(&ctx);
                        assert!(Instant::now() < deadline);
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    assert!(app.dem_profile_observation()["arrived"]
                        .as_array()
                        .unwrap()
                        .is_empty());
                }
                "new-provider-arrived" => {
                    control(
                        base,
                        "release",
                        &path(
                            &case["provider_variant"],
                            &case["provider_variant"]["tiles"][0],
                        ),
                        "path",
                    );
                    poll(&mut app, &ctx, |o| {
                        o["arrived"].as_array().unwrap().len() == 1
                    });
                }
                "repeat" => {}
                "recompute" => {
                    assert!(app.compute_survey_profile().unwrap());
                }
                "reader-cache-restart" => {
                    app.save_to_path(&root.join(format!("{name}-native.igz")));
                    drop(app);
                    app = setup(&root, case, base, &format!("{name}-native.igz"), &cache);
                    before = app.scene.clone();
                    undo = app.undo_stack.len();
                    redo = app.redo_stack.len();
                    assert!(app.compute_survey_profile().unwrap());
                    poll(&mut app, &ctx, |o| o["pending"] == 0);
                }
                "reader-cache-repeat" => {
                    assert!(app.compute_survey_profile().unwrap());
                    poll(&mut app, &ctx, |o| o["pending"] == 0);
                }
                _ => panic!("Unknown DEM phase {phase}"),
            }
            if let Some(csv) = expected["csv"].as_str() {
                let native = csv.strip_suffix(".csv").unwrap().to_owned() + "-native.csv";
                assert!(app.export_survey_profile(&root.join(native)).unwrap());
                if phase.starts_with("reader-cache") {
                    poll(&mut app, &ctx, |o| o["pending"] == 0);
                }
            }
            record(&app, name, phase, &expected["csv"], &mut rows);
            {
                assert_eq!(app.scene, before);
                assert_eq!((app.undo_stack.len(), app.redo_stack.len()), (undo, redo));
            }
        }
        poll(&mut app, &ctx, |o| o["pending"] == 0);
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
        app.survey_profile.limits = limits;
        let obs = app.dem_profile_observation();
        for _ in 0..4 {
            app.poll_dem_profile(&ctx);
        }
        assert!(std::sync::Arc::ptr_eq(
            &accepted,
            app.survey_profile.profile.as_ref().unwrap()
        ));
        assert_eq!(obs, app.dem_profile_observation());
        app.save_to_path(&root.join(format!("{name}-native.igz")));
        guards.push(json!({"case":name,"readonly":true,"budget_refusal_retained":true,"warm_owner_reused":true,"provider":app.dem_profile_observation(),"resident_bytes":app.survey_profile.resident_bytes()}));
    }
    std::fs::write(
        root.join("native-dem.json"),
        serde_json::to_vec_pretty(
            &json!({"cases":source["cases"],"states":rows,"source_pin":source["source_pin"]}),
        )
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        root.join("dem-guards.json"),
        serde_json::to_vec_pretty(&guards).unwrap(),
    )
    .unwrap();
}
