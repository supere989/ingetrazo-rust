use ingetrazo_core::render_blender::{Ambience, BlenderEngine, BlenderFound, RenderQuality};
use ingetrazo_gui::{IngeTrazoApp, blender::ResolutionPreset};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
fn poll(app: &mut IngeTrazoApp, ctx: &egui::Context, done: impl Fn(&IngeTrazoApp) -> bool) {
    let start = Instant::now();
    loop {
        app.blender_state.poll_events(ctx);
        if done(app) {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(90),
            "{}",
            app.blender_state.status_line
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn main() {
    let input = PathBuf::from(std::env::args().nth(1).unwrap());
    let out = PathBuf::from(std::env::args().nth(2).unwrap());
    std::fs::create_dir_all(&out).unwrap();
    let mut app = IngeTrazoApp::new();
    app.scene = ingetrazo_formats::load_file(&input).unwrap();
    let original = app.scene.clone();
    app.camera.target = glam::DVec3::ZERO;
    app.camera.distance = 5.;
    app.camera.yaw = 0.;
    app.camera.pitch = 0.;
    app.camera.perspective = true;
    app.blender_state.blender_found = Some(BlenderFound {
        command: std::env::var("INGETRAZO_TEST_BLENDER_COMMAND")
            .map(|s| serde_json::from_str::<Vec<String>>(&s).unwrap())
            .unwrap_or_else(|_| vec!["/usr/bin/blender".into(), "-t".into(), "2".into()]),
        where_desc: "actual devbox Blender".into(),
    });
    let ctx = egui::Context::default();
    app.blender_state.poll_events(&ctx);
    app.blender_state.res_preset = ResolutionPreset::Custom;
    app.blender_state.width = 128;
    app.blender_state.height = 96;
    app.blender_state.quality = RenderQuality::Draft;
    app.blender_state.ambience = Ambience::Overcast;
    app.blender_state.lights.clear();
    app.blender_state.ground = false;
    let mut observations = Vec::new();
    for engine in [BlenderEngine::Cycles, BlenderEngine::Eevee] {
        app.blender_state.engine = engine;
        app.blender_state
            .start_render_cached(&app.scene, &app.camera, Some(1));
        poll(&mut app, &ctx, |a| !a.blender_state.is_rendering);
        assert!(
            app.blender_state.status_line.contains("complete"),
            "{}",
            app.blender_state.status_line
        );
        let first_pid = app.blender_state.process_id.unwrap();
        let exports = app.blender_state.model_exports;
        let initial = app.blender_state.last_rendered_image_path.clone().unwrap();
        let name = engine.as_str();
        std::fs::copy(&initial, out.join(format!("{name}-first.png"))).unwrap();
        std::fs::copy(
            initial.parent().unwrap().join("job.json"),
            out.join(format!("{name}-job.json")),
        )
        .unwrap();
        std::fs::copy(
            initial.parent().unwrap().join("model.glb"),
            out.join(format!("{name}-model.glb")),
        )
        .unwrap();
        app.camera.yaw = 0.4;
        app.blender_state
            .start_render_cached(&app.scene, &app.camera, Some(1));
        poll(&mut app, &ctx, |a| !a.blender_state.is_rendering);
        assert_eq!(app.blender_state.process_id, Some(first_pid));
        assert_eq!(app.blender_state.model_exports, exports);
        let camera = app.blender_state.last_rendered_image_path.clone().unwrap();
        std::fs::copy(&camera, out.join(format!("{name}-camera.png"))).unwrap();
        assert_ne!(
            image::open(&initial).unwrap().to_rgba8(),
            image::open(&camera).unwrap().to_rgba8()
        );
        app.camera.yaw = 0.;
        app.blender_state
            .start_render_cached(&app.scene, &app.camera, Some(1));
        poll(&mut app, &ctx, |a| !a.blender_state.is_rendering);
        let restored = app.blender_state.last_rendered_image_path.clone().unwrap();
        assert_eq!(
            image::open(&initial).unwrap().to_rgba8(),
            image::open(&restored).unwrap().to_rgba8()
        );
        let saved = out.join(format!("{name}-saved.png"));
        app.blender_state.save_image(&saved).unwrap();
        assert_eq!(
            std::fs::read(saved).unwrap(),
            std::fs::read(&restored).unwrap()
        );
        observations.push(serde_json::json!({"engine":name,"pid":first_pid,"model_exports":exports,"completed":app.blender_state.completed_renders,"same_process_and_model":true,"restore_exact":true,"save_exact":true}));
        app.blender_state.cancel_render();
        poll(&mut app, &ctx, |a| a.blender_state.process_id.is_none());
        assert!(!Path::new(&format!("/proc/{first_pid}")).exists());
    }
    // Drive the same automatic-follow scheduler used by the application frame.
    app.camera.aspect = 4. / 3.;
    app.camera.target = glam::DVec3::new(0.1, 0.2, 0.3);
    app.blender_state.set_follow(true);
    let tick = |a: &mut IngeTrazoApp, time, held| a.blender_state.tick_follow(&a.scene, &a.camera, 1, time, held);
    let count = app.blender_state.completed_renders;
    tick(&mut app, 0., false); tick(&mut app, 0.69, false);
    assert!(!app.blender_state.is_rendering);
    tick(&mut app, 0.8, true); tick(&mut app, 1.4, false);
    assert!(!app.blender_state.is_rendering);
    tick(&mut app, 1.6, false);
    assert!(app.blender_state.is_rendering);
    app.camera.yaw=0.2; tick(&mut app, 2., false);
    app.camera.yaw=0.4; tick(&mut app, 3., false);
    poll(&mut app, &ctx, |a| !a.blender_state.is_rendering);
    assert_eq!(app.blender_state.completed_renders,count+1);
    let first = app.blender_state.last_rendered_image_path.clone().unwrap();
    let first_pixels=image::open(&first).unwrap().to_rgba8();
    assert_eq!(first_pixels.dimensions(),(960,720));
    let follow_pid=app.blender_state.process_id.unwrap();
    let follow_exports=app.blender_state.model_exports;
    std::fs::copy(&first,out.join("follow-first.png")).unwrap();
    tick(&mut app,3.8,false); poll(&mut app,&ctx,|a|!a.blender_state.is_rendering);
    assert_eq!(app.blender_state.process_id,Some(follow_pid));
    assert_eq!(app.blender_state.model_exports,follow_exports);
    assert_ne!(image::open(app.blender_state.last_rendered_image_path.as_ref().unwrap()).unwrap().to_rgba8(),first_pixels);
    app.camera.yaw=0.; tick(&mut app,4.,false); tick(&mut app,4.8,false);
    poll(&mut app,&ctx,|a|!a.blender_state.is_rendering);
    assert_eq!(image::open(app.blender_state.last_rendered_image_path.as_ref().unwrap()).unwrap().to_rgba8(),first_pixels);
    app.blender_state.sun_scale=1.5; tick(&mut app,5.6,false);
    poll(&mut app,&ctx,|a|!a.blender_state.is_rendering);
    assert_eq!(app.blender_state.model_exports,follow_exports+1);
    assert!(!Path::new(&format!("/proc/{follow_pid}")).exists());
    assert!(!first.exists(),"only the three most recent draft outputs are retained");
    let final_follow_pid=app.blender_state.process_id.unwrap();
    assert_eq!(app.blender_state.completed_renders,count+4);
    app.blender_state.set_follow(false);
    assert!(!Path::new(&format!("/proc/{final_follow_pid}")).exists());
    assert!(app.blender_state.process_id.is_none());
    app.blender_state.sun_scale=1.;
    app.camera.target=glam::DVec3::ZERO;
    app.camera.aspect=1.;
    app.blender_state.keep_blend = true;
    app.blender_state.start_render(&app.scene, &app.camera);
    poll(&mut app, &ctx, |a| {
        !a.blender_state.is_rendering && a.blender_state.process_id.is_none()
    });
    assert!(
        app.blender_state.status_line.contains("complete"),
        "{}",
        app.blender_state.status_line
    );
    let blend = app
        .blender_state
        .last_rendered_image_path
        .as_ref()
        .unwrap()
        .parent()
        .unwrap()
        .join("render.blend");
    assert!(std::fs::metadata(&blend).unwrap().len() > 0);
    std::fs::copy(&blend, out.join("render.blend")).unwrap();
    app.blender_state.keep_blend = false;
    // A real in-flight child is cancelled and reaped; previous successful output remains available.
    let previous = app.blender_state.last_rendered_image_path.clone();
    app.blender_state.width = 2048;
    app.blender_state.height = 2048;
    app.blender_state.quality = RenderQuality::High;
    app.blender_state.start_render(&app.scene, &app.camera);
    poll(&mut app, &ctx, |a| {
        a.blender_state.process_id.is_some() || !a.blender_state.is_rendering
    });
    let cancelled_pid = app.blender_state.process_id.unwrap();
    app.blender_state.cancel_render();
    poll(&mut app, &ctx, |a| {
        a.blender_state.process_id.is_none() && !a.blender_state.is_rendering
    });
    assert_eq!(previous, app.blender_state.last_rendered_image_path);
    assert!(!Path::new(&format!("/proc/{cancelled_pid}")).exists());
    app.blender_state.blender_found = Some(BlenderFound {
        command: vec!["/does/not/exist/blender".into()],
        where_desc: "missing control".into(),
    });
    app.blender_state.start_render(&app.scene, &app.camera);
    poll(&mut app, &ctx, |a| !a.blender_state.is_rendering);
    assert!(app.blender_state.status_line.contains("Error"));
    assert_eq!(previous, app.blender_state.last_rendered_image_path);
    assert_eq!(app.scene, original);
    assert!(app.undo_stack.is_empty() && app.redo_stack.is_empty());
    std::fs::write(out.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"engines":observations,"cancel_reaped":true,"spawn_failure_preserves_image":true,"document_and_history_unchanged":true,"completed_renders":app.blender_state.completed_renders,"keep_blend_one_shot":true,"follow_rest_mouse_hold_latest_camera":true,"follow_render_count":4,"follow_reused_pid":follow_pid,"follow_config_reload":true,"follow_three_file_retention":true,"follow_stop_reaped":true})).unwrap()).unwrap();
    println!("PASS real Blender application render/camera/restore/save/cancel/error controls");
}
