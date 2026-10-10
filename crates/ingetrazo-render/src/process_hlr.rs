// SPDX-License-Identifier: GPL-3.0-or-later
//! Persistent HLR processes: immutable mapped geometry, mapped results, descriptor-only IPC.
//! The native renderer owns this pool; no GPU command or full Scene crosses this boundary.
use glam::DVec3;
use memmap2::{Mmap, MmapMut};
use std::{
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver},
    },
    time::Duration,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
const MAGIC: &[u8; 8] = b"ITHLR001";
fn invalid(s: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, s)
}
fn word(b: &[u8], at: usize) -> io::Result<u64> {
    Ok(u64::from_le_bytes(
        b.get(at..at + 8)
            .ok_or_else(|| invalid("truncated shared buffer"))?
            .try_into()
            .unwrap(),
    ))
}
fn point(b: &[u8], at: usize) -> io::Result<DVec3> {
    Ok(DVec3::new(
        f64::from_bits(word(b, at)?),
        f64::from_bits(word(b, at + 8)?),
        f64::from_bits(word(b, at + 16)?),
    ))
}
fn put_point(f: &mut impl Write, p: DVec3) -> io::Result<()> {
    for v in p.to_array() {
        f.write_all(&v.to_le_bytes())?;
    }
    Ok(())
}
struct Worker {
    child: Child,
    input: ChildStdin,
    output: Receiver<io::Result<String>>,
    reader: Option<std::thread::JoinHandle<()>>,
    timeout: Duration,
    descriptor_bytes: u64,
    results: Mmap,
    path: PathBuf,
}
impl Worker {
    fn send(&mut self, command: &str) -> io::Result<()> {
        writeln!(self.input, "{command}")?;
        self.input.flush()?;
        self.descriptor_bytes += command.len() as u64 + 1;
        Ok(())
    }
    fn reply(&mut self) -> io::Result<String> {
        let line = self
            .output
            .recv_timeout(self.timeout)
            .map_err(|e| match e {
                mpsc::RecvTimeoutError::Timeout => io::Error::new(
                    io::ErrorKind::TimedOut,
                    "HLR worker response deadline exceeded",
                ),
                mpsc::RecvTimeoutError::Disconnected => {
                    invalid("HLR worker response channel closed")
                }
            })??;
        self.descriptor_bytes += line.len() as u64;
        if line.starts_with("ERR ") {
            return Err(invalid(line.trim()));
        }
        Ok(line)
    }
}
/// Workload telemetry reports real processes and binary bytes, not inferred speedups.
#[derive(Debug, Default, Clone)]
pub struct ProcessMetrics {
    pub workers: usize,
    pub tasks: usize,
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub descriptor_bytes: u64,
    pub elapsed: std::time::Duration,
}
/// Persistent children share one immutable projected geometry mapping per revision.
/// Output storage is explicitly bounded per worker; capacity exhaustion is an error.
pub struct HlrProcessPool {
    workers: Vec<Worker>,
    directory: PathBuf,
    revision: u64,
    pub metrics: ProcessMetrics,
    grain: usize,
    poisoned: bool,
}
impl HlrProcessPool {
    pub fn new(
        executable: &Path,
        count: usize,
        grain: usize,
        result_bytes: usize,
    ) -> io::Result<Self> {
        Self::new_with_timeout(
            executable,
            count,
            grain,
            result_bytes,
            Duration::from_secs(30),
        )
    }
    /// Response deadline applies to startup and every task descriptor exchange.
    pub fn new_with_timeout(
        executable: &Path,
        count: usize,
        grain: usize,
        result_bytes: usize,
        timeout: Duration,
    ) -> io::Result<Self> {
        if timeout.is_zero() {
            return Err(invalid("zero worker response deadline"));
        }
        if count == 0 || grain == 0 || result_bytes < 24 {
            return Err(invalid("invalid worker count, grain or output budget"));
        }
        let directory = std::env::temp_dir().join(format!(
            "ingetrazo-hlr-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory)?;
        let mut pool = Self {
            workers: vec![],
            directory,
            revision: 0,
            metrics: ProcessMetrics::default(),
            grain,
            poisoned: false,
        };
        for i in 0..count {
            let path = pool.directory.join(format!("result-{i}.bin"));
            let f = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&path)?;
            f.set_len(result_bytes as u64)?;
            // SAFETY: file length remains fixed; parent reads only after worker acknowledgement.
            let results = unsafe { Mmap::map(&f)? };
            let mut child = Command::new(executable)
                .arg("--hlr-worker")
                .arg(&path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()?;
            let input = child.stdin.take().unwrap();
            let mut pipe = BufReader::new(child.stdout.take().unwrap());
            let (sender, output) = mpsc::sync_channel(1);
            // One persistent descriptor reader per child makes deadlines portable.
            // Geometry/computation remains in child processes, never in this reader.
            let reader = std::thread::spawn(move || loop {
                let mut line = String::new();
                let read = std::io::Read::take(&mut pipe, 4097).read_line(&mut line);
                let reply = match read {
                    Ok(0) => Err(invalid("HLR worker exited")),
                    Ok(n) if n > 4096 || !line.ends_with('\n') => {
                        Err(invalid("oversized or incomplete worker descriptor"))
                    }
                    Ok(_) => Ok(line),
                    Err(e) => Err(e),
                };
                let failed = reply.is_err();
                if sender.send(reply).is_err() || failed {
                    break;
                }
            });
            pool.workers.push(Worker {
                child,
                input,
                output,
                reader: Some(reader),
                timeout,
                descriptor_bytes: 0,
                results,
                path,
            });
            if pool.workers.last_mut().unwrap().reply()?.trim() != "READY" {
                return Err(invalid("unexpected HLR worker handshake"));
            }
        }
        Ok(pool)
    }
    /// Preserves edge order and triangle interval order exactly, independent of task grain.
    pub fn intervals(
        &mut self,
        edges: &[(DVec3, DVec3)],
        triangles: &[[DVec3; 3]],
        epsilon: f64,
    ) -> io::Result<Vec<Vec<(f64, f64)>>> {
        if self.poisoned {
            return Err(invalid("HLR pool retired after worker/protocol failure"));
        }
        let result = self.intervals_inner(edges, triangles, epsilon);
        if result.is_err() {
            self.poisoned = true;
            for w in &mut self.workers {
                let _ = w.child.kill();
                let _ = w.child.wait();
            }
        }
        result
    }
    pub fn process_ids(&self) -> Vec<u32> {
        self.workers.iter().map(|w| w.child.id()).collect()
    }
    fn intervals_inner(
        &mut self,
        edges: &[(DVec3, DVec3)],
        triangles: &[[DVec3; 3]],
        epsilon: f64,
    ) -> io::Result<Vec<Vec<(f64, f64)>>> {
        if !epsilon.is_finite()
            || epsilon < 0.
            || edges.iter().any(|(a, b)| !a.is_finite() || !b.is_finite())
            || triangles.iter().flatten().any(|p| !p.is_finite())
        {
            return Err(invalid("nonfinite HLR input"));
        }
        let start = std::time::Instant::now();
        let descriptor_start = self.workers.iter().map(|w| w.descriptor_bytes).sum::<u64>();
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| invalid("revision overflow"))?;
        let path = self
            .directory
            .join(format!("geometry-{}.bin", self.revision));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let mut f = BufWriter::with_capacity(64 * 1024, file);
        f.write_all(MAGIC)?;
        f.write_all(&(edges.len() as u64).to_le_bytes())?;
        f.write_all(&(triangles.len() as u64).to_le_bytes())?;
        f.write_all(&epsilon.to_le_bytes())?;
        for &(a, b) in edges {
            put_point(&mut f, a)?;
            put_point(&mut f, b)?;
        }
        for tri in triangles {
            for &p in tri {
                put_point(&mut f, p)?;
            }
        }
        f.flush()?;
        let input_bytes = f.get_ref().metadata()?.len();
        drop(f);
        // Paths are generated internally, contain no protocol delimiters, and stay immutable.
        for w in &mut self.workers {
            w.send(&format!("LOAD {}", path.display()))?;
            if w.reply()?.trim() != "LOADED" {
                return Err(invalid("worker did not load geometry"));
            }
        }
        let mut result = vec![vec![]; edges.len()];
        let mut next = 0;
        let mut tasks = 0;
        let mut output_bytes = 0;
        while next < edges.len() {
            let mut pending = vec![];
            for (i, w) in self.workers.iter_mut().enumerate() {
                if next == edges.len() {
                    break;
                }
                let end = next.saturating_add(self.grain).min(edges.len());
                w.send(&format!("RUN {next} {end}"))?;
                pending.push((i, next, end));
                next = end;
                tasks += 1;
            }
            for (i, begin, end) in pending {
                let w = &mut self.workers[i];
                let reply = w.reply()?;
                let bytes = reply
                    .trim()
                    .strip_prefix("DONE ")
                    .ok_or_else(|| invalid("unexpected task reply"))?
                    .parse::<usize>()
                    .map_err(|_| invalid("bad result size"))?;
                let b = w
                    .results
                    .get(..bytes)
                    .ok_or_else(|| invalid("result exceeds shared output"))?;
                let mut at = 0;
                for row in &mut result[begin..end] {
                    let count = usize::try_from(word(b, at)?)
                        .map_err(|_| invalid("interval count overflow"))?;
                    at += 8;
                    if count > (bytes - at) / 16 {
                        return Err(invalid("corrupt interval count"));
                    }
                    for _ in 0..count {
                        let lo = f64::from_bits(word(b, at)?);
                        let hi = f64::from_bits(word(b, at + 8)?);
                        at += 16;
                        if !lo.is_finite() || !hi.is_finite() || lo < 0. || hi > 1. || lo >= hi {
                            return Err(invalid("invalid interval result"));
                        }
                        row.push((lo, hi));
                    }
                }
                if at != bytes {
                    return Err(invalid("trailing worker result"));
                }
                output_bytes += bytes as u64;
            }
        }
        // All workers have finished reading; replacement LOAD drops their prior mappings.
        for w in &mut self.workers {
            w.send("UNLOAD")?;
            if w.reply()?.trim() != "UNLOADED" {
                return Err(invalid("worker did not release geometry"));
            }
        }
        std::fs::remove_file(path)?;
        self.metrics = ProcessMetrics {
            workers: self.workers.len(),
            tasks,
            input_bytes,
            output_bytes,
            descriptor_bytes: self.workers.iter().map(|w| w.descriptor_bytes).sum::<u64>()
                - descriptor_start,
            elapsed: start.elapsed(),
        };
        Ok(result)
    }
}
impl Drop for HlrProcessPool {
    fn drop(&mut self) {
        for mut w in self.workers.drain(..) {
            let _ = w.child.kill();
            let _ = w.child.wait();
            // Disconnect bounded sender before joining, then unmap before Windows removal.
            drop(w.output);
            drop(w.input);
            if let Some(reader) = w.reader.take() {
                let _ = reader.join();
            }
            drop(w.results);
            let _ = std::fs::remove_file(&w.path);
        }
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
/// Entry point called before CLI/GUI initialization in the process executable.
pub fn worker_main(path: &Path) -> io::Result<()> {
    let f = OpenOptions::new().read(true).write(true).open(path)?;
    // SAFETY: fixed-length worker-exclusive result mapping, parent reads only after DONE.
    let mut output = unsafe { MmapMut::map_mut(&f)? };
    let mut geometry: Option<Mmap> = None;
    let mut index: Option<crate::triangle_index::TriangleIndex> = None;
    let mut candidates = vec![];
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    writeln!(stdout, "READY")?;
    stdout.flush()?;
    for line in stdin.lock().lines() {
        let line = line?;
        let response = (|| -> io::Result<String> {
            if line == "UNLOAD" {
                geometry = None;
                index = None;
                return Ok("UNLOADED".into());
            }
            if let Some(path) = line.strip_prefix("LOAD ") {
                let f = File::open(path)?;
                // SAFETY: parent publishes a closed immutable file and never changes its length/data.
                let map = unsafe { Mmap::map(&f)? };
                if map.get(..8) != Some(MAGIC) {
                    return Err(invalid("bad geometry magic"));
                }
                let n = word(&map, 8)?;
                let t = word(&map, 16)?;
                let size = n
                    .checked_mul(48)
                    .and_then(|v| t.checked_mul(72).and_then(|w| v.checked_add(w)))
                    .and_then(|v| v.checked_add(32))
                    .ok_or_else(|| invalid("geometry size overflow"))?;
                if size != map.len() as u64 {
                    return Err(invalid("invalid geometry size"));
                }
                let offset = 32 + n as usize * 48;
                index = Some(crate::triangle_index::TriangleIndex::new(
                    (0..t as usize).map(|i| {
                        let at = offset + i * 72;
                        // Header length validation proves these fixed records are in bounds.
                        [
                            point(&map, at).unwrap(),
                            point(&map, at + 24).unwrap(),
                            point(&map, at + 48).unwrap(),
                        ]
                    }),
                ));
                geometry = Some(map);
                return Ok("LOADED".into());
            }
            let mut parts = line.split_whitespace();
            if parts.next() != Some("RUN") {
                return Err(invalid("unknown task"));
            }
            let begin = parts
                .next()
                .ok_or_else(|| invalid("missing range"))?
                .parse::<usize>()
                .map_err(|_| invalid("bad range"))?;
            let end = parts
                .next()
                .ok_or_else(|| invalid("missing range"))?
                .parse::<usize>()
                .map_err(|_| invalid("bad range"))?;
            if parts.next().is_some() {
                return Err(invalid("trailing task fields"));
            }
            let b = geometry
                .as_ref()
                .ok_or_else(|| invalid("no geometry loaded"))?;
            let n = word(b, 8)? as usize;
            let _t = word(b, 16)? as usize;
            if begin > end || end > n {
                return Err(invalid("range exceeds geometry"));
            }
            let epsilon = f64::from_bits(word(b, 24)?);
            let mut at = 0;
            for edge in begin..end {
                if at + 8 > output.len() {
                    return Err(invalid("shared result capacity exceeded"));
                }
                let count_at = at;
                at += 8;
                let mut count = 0u64;
                let a = point(b, 32 + edge * 48)?;
                let c = point(b, 56 + edge * 48)?;
                index
                    .as_ref()
                    .ok_or_else(|| invalid("missing triangle index"))?
                    .query(
                        a.truncate().min(c.truncate()),
                        a.truncate().max(c.truncate()),
                        &mut candidates,
                    );
                for &i in &candidates {
                    let base = 32 + n * 48 + i * 72;
                    let tri = [point(b, base)?, point(b, base + 24)?, point(b, base + 48)?];
                    if let Some((lo, hi)) = crate::vector_hlr::occluded_interval(a, c, tri, epsilon)
                    {
                        if at + 16 > output.len() {
                            return Err(invalid("shared result capacity exceeded"));
                        }
                        output[at..at + 8].copy_from_slice(&lo.to_le_bytes());
                        output[at + 8..at + 16].copy_from_slice(&hi.to_le_bytes());
                        at += 16;
                        count += 1;
                    }
                }
                output[count_at..count_at + 8].copy_from_slice(&count.to_le_bytes());
            }
            Ok(format!("DONE {at}"))
        })();
        match response {
            Ok(s) => writeln!(stdout, "{s}")?,
            Err(e) => writeln!(stdout, "ERR {e}")?,
        }
        stdout.flush()?;
    }
    Ok(())
}
