//! Loopback framing only: model access stays with the thread that owns Bridge.
//! A timed-out queued request is cancelled before dispatch, never executed late.
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
    thread::{self, JoinHandle, ThreadId},
    time::{Duration, Instant},
};
const CLIENTS: usize = 4;
const POLL: Duration = Duration::from_millis(25);
#[derive(Clone, Copy)]
pub struct Limits {
    pub ui_timeout: Duration,
    pub max_frame_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            ui_timeout: Duration::from_secs(120),
            max_frame_bytes: 8 * 1024 * 1024,
        }
    }
}
struct Job {
    request: Value,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    reply: SyncSender<Value>,
}
pub struct Bridge {
    address: SocketAddr,
    owner: ThreadId,
    jobs: Receiver<Job>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
fn error(id: Value, message: &str) -> Value {
    json!({"id":id,"ok":false,"error":message})
}
impl Bridge {
    /// Never listens on an external interface. Port zero is useful for isolated tests.
    pub fn start(
        port: u16,
        limits: Limits,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> std::io::Result<Self> {
        if limits.max_frame_bytes == 0
            || limits.ui_timeout.is_zero()
            || Instant::now().checked_add(limits.ui_timeout).is_none()
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid bridge admission limits",
            ));
        }
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let (sender, jobs) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let wake = Arc::new(wake);
        // Fixed persistent pool: no client or request spawns a thread. Each slot
        // admits one connection; excess clients receive an explicit refusal.
        let mut slots = Vec::with_capacity(CLIENTS);
        let mut workers = Vec::with_capacity(CLIENTS);
        for index in 0..CLIENTS {
            let (connections, incoming) = mpsc::sync_channel::<TcpStream>(0);
            let sender = sender.clone();
            let stop = stopped.clone();
            let wake = wake.clone();
            match thread::Builder::new()
                .name(format!("ingetrazo-ai-client-{index}"))
                .spawn(move || {
                    while !stop.load(Ordering::Acquire) {
                        match incoming.recv_timeout(POLL) {
                            Ok(connection) => serve(connection, &sender, &stop, limits, &*wake),
                            Err(mpsc::RecvTimeoutError::Timeout) => continue,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    }
                }) {
                Ok(worker) => {
                    slots.push(connections);
                    workers.push(worker);
                }
                Err(error) => {
                    stopped.store(true, Ordering::Release);
                    drop(slots);
                    for worker in workers {
                        let _ = worker.join();
                    }
                    return Err(error);
                }
            }
        }
        let worker = thread::Builder::new()
            .name("ingetrazo-ai-bridge".into())
            .spawn(move || {
                while !stopped.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((connection, _)) => {
                            let mut pending = Some(connection);
                            for slot in &slots {
                                match slot.try_send(pending.take().expect("unadmitted connection")) {
                                    Ok(()) => break,
                                    Err(mpsc::TrySendError::Full(returned) | mpsc::TrySendError::Disconnected(returned)) => pending = Some(returned),
                                }
                            }
                            if let Some(mut connection) = pending {
                                let _ = connection.set_write_timeout(Some(POLL));
                                let _ = connection.write(b"{\"id\":null,\"ok\":false,\"error\":\"bridge client capacity reached\"}\n");
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(POLL),
                        Err(_) => break,
                    }
                }
                stopped.store(true, Ordering::Release);
                drop(slots);
                for worker in workers { let _ = worker.join(); }
            })?;
        Ok(Self {
            address,
            owner: thread::current().id(),
            jobs,
            stop,
            worker: Some(worker),
        })
    }
    pub fn address(&self) -> SocketAddr {
        self.address
    }
    /// At most one pending operation; caller remains the sole document owner.
    pub fn poll(&mut self, execute: impl FnOnce(&Value) -> Value) -> Result<bool, &'static str> {
        if thread::current().id() != self.owner {
            return Err("bridge dispatch requires its owning UI thread");
        }
        let Ok(job) = self.jobs.try_recv() else {
            return Ok(false);
        };
        if self.stop.load(Ordering::Acquire)
            || job.cancelled.load(Ordering::Acquire)
            || Instant::now() >= job.deadline
        {
            return Ok(false);
        }
        let reply = execute(&job.request);
        let _ = job.reply.try_send(reply);
        Ok(true)
    }
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        while let Ok(job) = self.jobs.try_recv() {
            job.cancelled.store(true, Ordering::Release);
        }
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop();
    }
}
fn serve(
    mut connection: TcpStream,
    sender: &SyncSender<Job>,
    stop: &AtomicBool,
    limits: Limits,
    wake: &dyn Fn(),
) {
    if connection.set_read_timeout(Some(POLL)).is_err()
        || connection.set_write_timeout(Some(POLL)).is_err()
    {
        return;
    }
    let mut frame = Vec::new();
    let mut bytes = [0u8; 65536];
    while !stop.load(Ordering::Acquire) {
        let size = match connection.read(&mut bytes) {
            Ok(0) => return,
            Ok(n) => n,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue
            }
            Err(_) => return,
        };
        for &byte in &bytes[..size] {
            if byte == b'\n' {
                if frame.iter().all(u8::is_ascii_whitespace) {
                    frame.clear();
                    continue;
                }
                let reply = dispatch(&frame, sender, stop, limits, wake);
                frame.clear();
                let mut encoded = match serde_json::to_vec(&reply) {
                    Ok(v) => v,
                    Err(_) => return,
                };
                encoded.push(b'\n');
                if !write_reply(&mut connection, &encoded, stop) {
                    return;
                }
            } else {
                if frame.len() >= limits.max_frame_bytes {
                    let _=connection.write_all(b"{\"id\":null,\"ok\":false,\"error\":\"bridge frame exceeds configured admission limit\"}\n");
                    return;
                }
                frame.push(byte);
            }
        }
    }
}
fn write_reply(connection: &mut TcpStream, mut bytes: &[u8], stop: &AtomicBool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !bytes.is_empty() {
        if stop.load(Ordering::Acquire) || Instant::now() >= deadline {
            return false;
        }
        match connection.write(bytes) {
            Ok(0) => return false,
            Ok(n) => bytes = &bytes[n..],
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return false,
        }
    }
    true
}
fn dispatch(
    frame: &[u8],
    sender: &SyncSender<Job>,
    stop: &AtomicBool,
    limits: Limits,
    wake: &dyn Fn(),
) -> Value {
    let request: Value = match serde_json::from_slice(frame) {
        Ok(Value::Object(v)) => Value::Object(v),
        Ok(_) => return error(Value::Null, "request must be an object"),
        Err(_) => return error(Value::Null, "bad json"),
    };
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let (reply, result) = mpsc::sync_channel(1);
    let cancelled = Arc::new(AtomicBool::new(false));
    let deadline = Instant::now() + limits.ui_timeout;
    if sender
        .try_send(Job {
            request,
            deadline,
            cancelled: cancelled.clone(),
            reply,
        })
        .is_err()
    {
        return error(id, "UI request queue is busy");
    }
    wake();
    loop {
        if stop.load(Ordering::Acquire) {
            cancelled.store(true, Ordering::Release);
            return error(id, "bridge stopped");
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            cancelled.store(true, Ordering::Release);
            return error(id, "timed out on the UI thread");
        }
        match result.recv_timeout(remaining.min(POLL)) {
            Ok(reply) => return reply,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return error(id, "UI request cancelled"),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn read_line(stream: &mut TcpStream) -> Value {
        let mut line = Vec::new();
        let mut b = [0];
        loop {
            stream.read_exact(&mut b).unwrap();
            if b[0] == b'\n' {
                break;
            }
            line.push(b[0]);
        }
        serde_json::from_slice(&line).unwrap()
    }
    #[test]
    fn persistent_clients_are_bounded_without_idle_monopolization() {
        let mut bridge = Bridge::start(0, Limits::default(), || {}).unwrap();
        let mut clients = Vec::new();
        for _ in 0..CLIENTS {
            let mut client = TcpStream::connect(bridge.address()).unwrap();
            client
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            client.write_all(b"bad\n").unwrap();
            assert_eq!(read_line(&mut client)["error"], "bad json");
            clients.push(client);
        }
        // Three idle persistent peers cannot starve the fourth active peer.
        clients[3]
            .write_all(b"{\"id\":19,\"tool\":\"query_model\"}\n")
            .unwrap();
        let until = Instant::now() + Duration::from_secs(1);
        while !bridge
            .poll(|req| json!({"id":req["id"],"ok":true}))
            .unwrap()
        {
            assert!(Instant::now() < until);
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(read_line(&mut clients[3])["id"], 19);
        let mut excess = TcpStream::connect(bridge.address()).unwrap();
        excess
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        assert_eq!(
            read_line(&mut excess)["error"],
            "bridge client capacity reached"
        );
        let start = Instant::now();
        bridge.stop();
        assert!(start.elapsed() < Duration::from_secs(1));
    }
    #[test]
    fn framing_dispatch_and_cancellation() {
        let mut bridge = Bridge::start(
            0,
            Limits {
                ui_timeout: Duration::from_millis(100),
                max_frame_bytes: 1024,
            },
            || {},
        )
        .unwrap();
        let mut client = TcpStream::connect(bridge.address()).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        client.write_all(b"bad\n").unwrap();
        assert_eq!(read_line(&mut client)["error"], "bad json");
        client
            .write_all(b"{\"id\":7,\"tool\":\"query_model\"}\n")
            .unwrap();
        let until = Instant::now() + Duration::from_secs(1);
        while !bridge
            .poll(|request| json!({"id":request["id"],"ok":true,"result":{"faces":3}}))
            .unwrap()
        {
            assert!(Instant::now() < until);
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(read_line(&mut client)["result"]["faces"], 3);
        client.write_all(b"{\"id\":8,\"tool\":\"undo\"}\n").unwrap();
        assert_eq!(
            read_line(&mut client)["error"],
            "timed out on the UI thread"
        );
        let mut executions = 0;
        assert!(!bridge
            .poll(|_| {
                executions += 1;
                json!({})
            })
            .unwrap());
        assert_eq!(executions, 0);
        let start = Instant::now();
        bridge.stop();
        assert!(start.elapsed() < Duration::from_secs(1));
    }
    #[test]
    fn foreign_thread_cannot_dispatch() {
        let mut bridge = Bridge::start(0, Limits::default(), || {}).unwrap();
        thread::spawn(move || {
            assert_eq!(
                bridge.poll(|_| panic!("foreign owner executed")),
                Err("bridge dispatch requires its owning UI thread")
            );
        })
        .join()
        .unwrap();
    }
    #[test]
    fn bounded_frame_and_pending_stop() {
        let mut bridge = Bridge::start(
            0,
            Limits {
                ui_timeout: Duration::from_secs(120),
                max_frame_bytes: 8,
            },
            || {},
        )
        .unwrap();
        let mut client = TcpStream::connect(bridge.address()).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        client.write_all(b"123456789").unwrap();
        assert!(read_line(&mut client)["error"]
            .as_str()
            .unwrap()
            .contains("admission"));
        bridge.stop();
        let mut bridge = Bridge::start(0, Limits::default(), || {}).unwrap();
        let mut client = TcpStream::connect(bridge.address()).unwrap();
        client.write_all(b"{\"id\":9}\n").unwrap();
        thread::sleep(Duration::from_millis(40));
        let start = Instant::now();
        bridge.stop();
        assert!(start.elapsed() < Duration::from_secs(1));
        let mut executions = 0;
        assert!(!bridge
            .poll(|_| {
                executions += 1;
                json!({})
            })
            .unwrap());
        assert_eq!(executions, 0);
    }
}
