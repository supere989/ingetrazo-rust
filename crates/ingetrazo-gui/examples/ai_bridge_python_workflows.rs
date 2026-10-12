//! Actual loopback transport and full App UI-thread transactional dispatch.
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    net::{Shutdown, TcpStream},
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};
fn frame(app: &mut IngeTrazoApp, ctx: &egui::Context, tick: &mut f64) {
    *tick += 0.01;
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1280., 800.),
        )),
        time: Some(*tick),
        ..Default::default()
    };
    let _ = ctx.run(raw, |ctx| app.update_ui(ctx));
}
fn receive(
    app: &mut IngeTrazoApp,
    ctx: &egui::Context,
    tick: &mut f64,
    rx: &mpsc::Receiver<Result<Value, String>>,
) -> Result<Value, String> {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        frame(app, ctx, tick);
        match rx.try_recv() {
            Ok(reply) => return reply,
            Err(mpsc::TryRecvError::Disconnected) => return Err("Socket reader closed".into()),
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if Instant::now() >= end {
            return Err("Actual UI socket dispatch timeout".into());
        }
        std::thread::yield_now();
    }
}
fn observe(app: &IngeTrazoApp) -> Value {
    let faces: Vec<_> = app.scene.mesh.faces.iter().map(|face| json!({"outer":face.vertices.iter().map(|i|app.scene.mesh.vertices[*i].position.to_array()).collect::<Vec<_>>(),"holes":face.holes.iter().map(|h|h.iter().map(|i|app.scene.mesh.vertices[*i].position.to_array()).collect::<Vec<_>>()).collect::<Vec<_>>(),"attrs":face.attrs})).collect();
    json!({"faces":faces,"undo":app.undo_stack.len(),"redo":app.redo_stack.len(),"groups":app.scene.groups.len(),"layers":app.scene.layers.len()})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = PathBuf::from(std::env::args().nth(1).ok_or("SOURCE required")?).canonicalize()?;
    let out = PathBuf::from(std::env::args().nth(2).ok_or("OUTPUT required")?);
    std::fs::create_dir_all(&out)?;
    let original: Value = serde_json::from_slice(&std::fs::read(
        source.join("original-ai-bridge-python.json"),
    )?)?;
    assert_eq!(
        original["source_pin"],
        "6be29fe437a7cd992d4e079f71720821b25216b9"
    );
    let mut app = IngeTrazoApp::new();
    app.native_gpu_viewport_enabled = true;
    #[cfg(feature = "engineering-python")]
    {
        app.engineering_python_enabled = true;
    }
    #[cfg(not(feature = "engineering-python"))]
    return Err("engineering-python feature required".into());
    let ctx = egui::Context::default();
    let mut tick = 0.;
    let port = app.start_ai_bridge(&ctx, 0)?;
    let mut socket = TcpStream::connect(("127.0.0.1", port))?;
    socket.set_write_timeout(Some(Duration::from_secs(3)))?;
    let reader = socket.try_clone()?;
    reader.set_read_timeout(Some(Duration::from_secs(10)))?;
    let (tx, rx) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let reply = serde_json::from_str(&line).map_err(|e| e.to_string());
                    if tx.send(reply).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = tx.send(Err(error.to_string()));
                    break;
                }
            }
        }
    });
    let result = (|| -> Result<Value, Box<dyn std::error::Error>> {
        let mut report = json!({"source_pin":original["source_pin"],"cases":original["cases"],"states":[],"controls":{}});
        for case in original["cases"].as_array().ok_or("cases required")? {
            let name = case["name"].as_str().ok_or("case name required")?;
            let requests = case["requests"].as_array().ok_or("requests required")?;
            if name == "fragmented-and-ordered-requests" {
                let mut bytes = Vec::new();
                for request in requests {
                    bytes.extend(serde_json::to_vec(request)?);
                    bytes.push(b'\n');
                }
                socket.write_all(&bytes[..7])?;
                frame(&mut app, &ctx, &mut tick);
                socket.write_all(&bytes[7..])?;
            }
            for (index, request) in requests.iter().enumerate() {
                if name != "fragmented-and-ordered-requests" {
                    if request.is_null() {
                        socket.write_all(b"{bad-json}\n")?
                    } else {
                        socket.write_all(&serde_json::to_vec(request)?)?;
                        socket.write_all(b"\n")?
                    }
                }
                let reply = receive(&mut app, &ctx, &mut tick, &rx)?;
                report["states"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"case":name,"index":index,"reply":reply,"scene":observe(&app)}));
                std::fs::write(
                    out.join("native-ai-bridge.partial.json"),
                    serde_json::to_vec_pretty(&report)?,
                )?;
            }
        }
        let before = app.scene.clone();
        let undo = app.undo_stack.len();
        let redo = app.redo_stack.len();
        // Caught output overflow must still poison the existing transaction.
        let request = json!({"id":"caught-overflow","tool":"run_python","args":{"code":"try:\n print('x'*1100000)\nexcept MemoryError:\n pass\nmesh.add_face([QVector3D(4,0,0),QVector3D(5,0,0),QVector3D(4,1,0)])"}});
        socket.write_all(&serde_json::to_vec(&request)?)?;
        socket.write_all(b"\n")?;
        let overflow = receive(&mut app, &ctx, &mut tick, &rx)?;
        assert_eq!(overflow["ok"], false);
        assert_eq!(app.scene, before);
        assert_eq!((app.undo_stack.len(), app.redo_stack.len()), (undo, redo));
        #[cfg(feature = "engineering-python")]
        {
            app.engineering_python_enabled = false;
        }
        let request = json!({"id":"disabled","tool":"run_python","args":{"code":"1"}});
        socket.write_all(&serde_json::to_vec(&request)?)?;
        socket.write_all(b"\n")?;
        let disabled = receive(&mut app, &ctx, &mut tick, &rx)?;
        assert_eq!(disabled["ok"], false);
        assert_eq!(app.scene, before);
        #[cfg(feature = "engineering-python")]
        {
            app.engineering_python_enabled = true;
        }
        app.show_save_dialog = true;
        socket.write_all(&serde_json::to_vec(&request)?)?;
        socket.write_all(b"\n")?;
        let modal = receive(&mut app, &ctx, &mut tick, &rx)?;
        assert_eq!(modal["ok"], false);
        assert_eq!(app.scene, before);
        app.show_save_dialog = false;
        report["controls"] = json!({"actual_socket":true,"actual_ui_poll":true,"caught_overflow_atomic":true,"disabled_refused":true,"modal_refused":true,"worker_joined":false});
        report["control_replies"] = json!({"overflow":overflow,"disabled":disabled,"modal":modal});
        Ok(report)
    })();
    app.stop_ai_bridge();
    socket.shutdown(Shutdown::Both)?;
    thread.join().map_err(|_| "Owned socket reader panicked")?;
    let mut report = result?;
    report["controls"]["worker_joined"] = json!(true);
    std::fs::write(
        out.join("native-ai-bridge-python.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
