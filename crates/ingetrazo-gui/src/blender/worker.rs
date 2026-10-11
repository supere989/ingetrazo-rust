//! One owned Blender process, drained pipes and bounded progress storage.
//! Serving jobs retain imported geometry; subsequent requests contain only a camera.
use super::BlenderRenderMsg;
use std::{
    collections::VecDeque,
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::Duration,
};

#[cfg(unix)]
fn stop_descendants(child: &Child) {
    // Command::process_group creates a new group owned by this job.
    unsafe {
        libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
    }
}
#[cfg(not(unix))]
fn stop_descendants(_: &Child) {}
fn stop_child(child: &mut Child) {
    stop_descendants(child);
    let _ = child.kill();
    let _ = child.wait();
}

pub(super) struct Worker {
    requests: SyncSender<serde_json::Value>,
    stop: Arc<AtomicBool>,
    progress: Arc<Mutex<Option<(f32, String)>>>,
    join: Option<thread::JoinHandle<()>>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
impl Worker {
    pub fn progress(&self) -> Option<(f32, String)> {
        self.progress.lock().unwrap().take()
    }
    pub fn cancel(&self) {
        self.stop.store(true, Ordering::Release);
    }
    pub fn request(&self, value: serde_json::Value) -> Result<(), String> {
        self.requests
            .try_send(value)
            .map_err(|e| format!("Blender request refused: {e}"))
    }
    pub fn launch(
        args: Vec<String>,
        output: PathBuf,
        serving: bool,
        first: serde_json::Value,
        ctx: Option<egui::Context>,
    ) -> (Self, Receiver<BlenderRenderMsg>) {
        let (request_tx, request_rx) = mpsc::sync_channel(1);
        let (events_tx, events_rx) = mpsc::channel();
        let progress = Arc::new(Mutex::new(None));
        let progress_worker = progress.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_worker = stop.clone();
        let join = thread::spawn(move || {
            let emit = |event| {
                match event {
                    BlenderRenderMsg::Progress(value, line) => {
                        *progress_worker.lock().unwrap() = Some((value, line));
                    }
                    _ => {
                        let _ = events_tx.send(event);
                    }
                }
                if let Some(ctx) = &ctx {
                    ctx.request_repaint();
                }
            };
            let mut cmd = Command::new(&args[0]);
            cmd.args(&args[1..])
                .env_clear()
                .envs(ingetrazo_core::render_blender::clean_env())
                .stdin(if serving {
                    Stdio::piped()
                } else {
                    Stdio::null()
                })
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                cmd.process_group(0);
            }
            let mut child = match cmd.spawn() {
                Ok(child) => child,
                Err(e) => {
                    emit(BlenderRenderMsg::Error(e.to_string()));
                    return;
                }
            };
            emit(BlenderRenderMsg::Started(child.id()));
            let mut input = child.stdin.take();
            let (line_tx, line_rx) = mpsc::sync_channel(64);
            fn drain(
                read: impl Read + Send + 'static,
                tx: SyncSender<String>,
            ) -> thread::JoinHandle<()> {
                thread::spawn(move || {
                    let mut read = BufReader::new(read);
                    let mut line = Vec::new();
                    loop {
                        // Limit one diagnostic line independently of the pipe capacity.
                        let mut part = (&mut read).take(16 * 1024);
                        match part.read_until(b'\n', &mut line) {
                            Ok(0) | Err(_) => break,
                            Ok(_) => {
                                let text = String::from_utf8_lossy(&line).trim_end().to_string();
                                if tx.send(text).is_err() {
                                    break;
                                }
                                line.clear();
                            }
                        }
                    }
                })
            }
            let readers = [
                drain(child.stdout.take().unwrap(), line_tx.clone()),
                drain(child.stderr.take().unwrap(), line_tx),
            ];
            let mut tail = VecDeque::new();
            let mut pending = Some(first);
            let mut active = Some(output);
            let mut ready = !serving;
            let mut fatal = None;
            let mut finished = false;
            loop {
                if stop_worker.load(Ordering::Acquire) {
                    stop_child(&mut child);
                    emit(BlenderRenderMsg::Cancelled);
                    break;
                }
                // A fixed channel bounds producer memory; a fixed drain count preserves cancellation liveness.
                for line in line_rx.try_iter().take(128) {
                    if tail.len() == 40 {
                        tail.pop_front();
                    }
                    tail.push_back(line.clone());
                    if line == "INGETRAZO ready" {
                        ready = true;
                    }
                    if let Some(progress) = ingetrazo_core::render_blender::progress_of(&line)
                        .filter(|_| active.is_some())
                    {
                        emit(BlenderRenderMsg::Progress(progress, line.clone()));
                    }
                    if serving && line.starts_with("INGETRAZO done ") {
                        if let Some(path) = active.take() {
                            if line.strip_prefix("INGETRAZO done ") == path.to_str()
                                && path.is_file()
                            {
                                emit(BlenderRenderMsg::Done(path));
                            } else {
                                fatal = Some("Blender returned an unexpected output".to_string());
                            }
                        }
                    }
                }
                if fatal.is_some() {
                    stop_child(&mut child);
                    break;
                }
                if ready && serving {
                    if pending.is_none() && active.is_none() {
                        pending = request_rx.try_recv().ok();
                    }
                    if let Some(request) = pending.take() {
                        active = request["output"].as_str().map(PathBuf::from);
                        let line = request.to_string() + "\n";
                        if let Err(e) = input
                            .as_mut()
                            .unwrap()
                            .write_all(line.as_bytes())
                            .and_then(|_| input.as_mut().unwrap().flush())
                        {
                            fatal = Some(format!("Blender request failed: {e}"));
                            stop_child(&mut child);
                            break;
                        }
                    }
                }
                match child.try_wait() {
                    Ok(Some(status)) => {
                        stop_descendants(&child);
                        // Child is reaped. Drain remaining diagnostics after closing their producers.
                        if !serving
                            && status.success()
                            && active.as_ref().is_some_and(|p| p.is_file())
                        {
                            emit(BlenderRenderMsg::Done(active.take().unwrap()));
                            finished = true;
                        } else {
                            fatal = Some(format!("Blender exited with {status}"));
                        }
                        break;
                    }
                    Ok(None) => (),
                    Err(e) => {
                        fatal = Some(e.to_string());
                        stop_child(&mut child);
                        break;
                    }
                }
                thread::sleep(Duration::from_millis(10));
            }
            // Drop receiver before joining: a diagnostic writer cannot strand process cleanup.
            for line in line_rx.try_iter() {
                if tail.len() == 40 {
                    tail.pop_front();
                }
                tail.push_back(line);
            }
            drop(line_rx);
            drop(input);
            for reader in readers {
                let _ = reader.join();
            }
            if let Some(error) = fatal {
                emit(BlenderRenderMsg::Error(format!(
                    "{error}\n{}",
                    tail.into_iter().collect::<Vec<_>>().join("\n")
                )));
            }
            emit(BlenderRenderMsg::Stopped {
                completed: finished,
            });
        });
        (
            Self {
                requests: request_tx,
                stop,
                progress,
                join: Some(join),
            },
            events_rx,
        )
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn drop_reaps_owned_child_with_inherited_descendant_pipes() {
        let (worker, events) = Worker::launch(
            vec![
                "/bin/sh".into(),
                "-c".into(),
                "sleep 30 & echo 'INGETRAZO ready'; wait".into(),
            ],
            PathBuf::from("unused"),
            true,
            serde_json::json!({"output":"unused"}),
            None,
        );
        let pid = match events.recv_timeout(Duration::from_secs(2)).unwrap() {
            BlenderRenderMsg::Started(pid) => pid,
            _ => panic!("expected Started"),
        };
        thread::sleep(Duration::from_millis(50));
        let start = std::time::Instant::now();
        drop(worker);
        assert!(start.elapsed() < Duration::from_secs(2));
        assert_eq!(unsafe { libc::kill(pid as libc::pid_t, 0) }, -1);
    }
}
