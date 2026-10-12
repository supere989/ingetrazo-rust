//! Lazy persistent native DEM transport. Source-cache resources are independent
//! of document snapshots; generations retire requests without joining on a UI
//! close/document switch. Drop owns and joins workers (DNS remains OS-bound).
use ingetrazo_formats::dem_profile::{Encoding, Field, Tile};
use std::{
    collections::HashSet,
    io::Read,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Duration,
};
pub const CONTROL_RESERVATION: usize = 274 * (2 * 4352 + 512) + 8 * 8192 + 4096;
#[derive(Clone, Debug)]
pub struct Source {
    pub id: String,
    pub url_template: String,
    pub encoding: Encoding,
    pub max_zoom: u8,
}
impl Default for Source {
    fn default() -> Self {
        Self {
            id: "aws_terrarium".into(),
            url_template: "https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{z}/{x}/{y}.png"
                .into(),
            encoding: Encoding::Terrarium,
            max_zoom: 15,
        }
    }
}
impl Source {
    pub fn url(&self, tile: Tile) -> String {
        self.url_template
            .replace("{z}", &tile.zoom.to_string())
            .replace("{x}", &tile.x.to_string())
            .replace("{y}", &tile.y.to_string())
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.id.len() > 256 || self.url_template.len() > 4096 {
            return Err("DEM provider control data exceeds admitted size".into());
        }
        if self.id.is_empty()
            || self
                .id
                .bytes()
                .any(|b| !b.is_ascii_alphanumeric() && !b"_-".contains(&b))
        {
            return Err("DEM source cache id is not supported".into());
        }
        if !self.url_template.starts_with("https://") && !self.url_template.starts_with("http://") {
            return Err("DEM source must use HTTP or HTTPS".into());
        }
        Ok(())
    }
}
struct Job {
    tile: Tile,
    generation: u64,
    url: String,
    max_bytes: usize,
    deadline: Duration,
    source: Source,
    cache: Arc<Mutex<DiskCache>>,
    decoded_limit: usize,
}
pub struct Completion {
    pub tile: Tile,
    pub generation: u64,
    pub result: Result<Ready, String>,
}
pub struct Ready {
    pub field: Arc<Field>,
    pub cache_hit: bool,
    pub encoded_bytes: usize,
}
pub struct Transport {
    request: Option<mpsc::SyncSender<Job>>,
    result: Option<mpsc::Receiver<Completion>>,
    generation: Arc<AtomicU64>,
    workers: Vec<thread::JoinHandle<()>>,
    wake: Arc<Mutex<Option<egui::Context>>>,
    pub pending: HashSet<Tile>,
    deadline: Duration,
    max_encoded_bytes: usize,
    encoded_reservation_bytes: usize,
    decoded_reservation_bytes: usize,
}
impl Transport {
    pub fn new(
        deadline: Duration,
        encoded_reservation_bytes: usize,
        decoded_reservation_bytes: usize,
    ) -> Result<Self, String> {
        // Eight active bodies, eight completed queued bodies, one currently
        // polled body; worker stack read buffers are also reserved explicitly.
        const SCRATCH: usize = CONTROL_RESERVATION;
        let max_encoded_bytes = encoded_reservation_bytes
            .checked_sub(SCRATCH)
            .ok_or("DEM transport scratch exceeds configured CPU budget")?
            / 17;
        if max_encoded_bytes == 0 || decoded_reservation_bytes / 17 == 0 {
            return Err("DEM transport response admission is empty".into());
        }
        let mut pending = HashSet::new();
        pending
            .try_reserve(274)
            .map_err(|_| "DEM pending index admission refused")?;
        let (tx, rx) = mpsc::sync_channel::<Job>(257);
        let (ready_tx, ready_rx) = mpsc::sync_channel(8);
        let rx = Arc::new(Mutex::new(rx));
        let generation = Arc::new(AtomicU64::new(1));
        let mut workers = Vec::new();
        let wake: Arc<Mutex<Option<egui::Context>>> = Arc::new(Mutex::new(None));
        // Agent clones share connection-pool ownership across the fixed workers.
        let agent = ureq::AgentBuilder::new()
            .timeout(deadline)
            .redirects(0)
            .build();
        for _ in 0..8 {
            let rx = rx.clone();
            let ready = ready_tx.clone();
            let epoch = generation.clone();
            let agent = agent.clone();
            let wake = wake.clone();
            workers.push(thread::spawn(move || loop {
                let job = match rx.lock().unwrap_or_else(|e| e.into_inner()).recv() {
                    Ok(job) => job,
                    Err(_) => break,
                };
                if epoch.load(Ordering::Acquire) != job.generation {
                    continue;
                }
                let result = tile(&agent, &job);
                if epoch.load(Ordering::Acquire) != job.generation {
                    continue;
                }
                if ready
                    .send(Completion {
                        tile: job.tile,
                        generation: job.generation,
                        result,
                    })
                    .is_err()
                {
                    break;
                }
                if let Some(ctx) = wake.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                    ctx.request_repaint();
                }
            }));
        }
        Ok(Self {
            request: Some(tx),
            result: Some(ready_rx),
            generation,
            workers,
            wake,
            pending,
            deadline,
            max_encoded_bytes,
            encoded_reservation_bytes,
            decoded_reservation_bytes,
        })
    }
    pub fn set_wake(&self, ctx: &egui::Context) {
        *self.wake.lock().unwrap_or_else(|e| e.into_inner()) = Some(ctx.clone());
    }
    pub fn encoded_reservation_bytes(&self) -> usize {
        self.encoded_reservation_bytes
    }
    pub fn decoded_reservation_bytes(&self) -> usize {
        self.decoded_reservation_bytes
    }
    pub fn decoded_limit(&self) -> usize {
        self.decoded_reservation_bytes / 17
    }
    pub fn max_encoded_bytes(&self) -> usize {
        self.max_encoded_bytes
    }
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }
    pub fn deadline(&self) -> Duration {
        self.deadline
    }
    pub fn request(
        &mut self,
        source: &Source,
        tile: Tile,
        max_bytes: usize,
        cache: Arc<Mutex<DiskCache>>,
    ) -> Result<bool, String> {
        if tile.zoom > source.max_zoom {
            return Ok(false);
        }
        if self.pending.contains(&tile) {
            return Ok(false);
        }
        source.validate()?;
        if self.pending.len() >= 274 {
            return Err("DEM request queue admission refused".into());
        }
        let job = Job {
            tile,
            generation: self.generation(),
            url: source.url(tile),
            max_bytes: max_bytes.min(self.max_encoded_bytes),
            deadline: self.deadline,
            source: source.clone(),
            cache,
            decoded_limit: self.decoded_limit(),
        };
        self.request
            .as_ref()
            .ok_or("DEM transport is closed")?
            .try_send(job)
            .map_err(|_| "DEM request queue admission refused")?;
        self.pending.insert(tile);
        Ok(true)
    }
    pub fn poll(&mut self) -> Option<Completion> {
        loop {
            let result = self.result.as_ref()?.try_recv().ok()?;
            if result.generation != self.generation() {
                continue;
            }
            self.pending.remove(&result.tile);
            return Some(result);
        }
    }
    pub fn cancel_generation(&mut self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.pending.clear();
    }
}
impl Drop for Transport {
    fn drop(&mut self) {
        self.cancel_generation();
        drop(self.request.take());
        drop(self.result.take());
        // Dropping the result receiver releases blocked terminal sends before joins.
        // HTTP body/connect waits obey deadline; system DNS may exceed it, as ureq
        // documents. No request subprocess or unowned detached thread is introduced.
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}
pub fn admission_error(e: &str) -> bool {
    e.contains("budget")
        || e.contains("allocation")
        || e.contains("overflow")
        || e.starts_with("DEM admission:")
}
pub fn decode(bytes: &[u8], encoding: Encoding, limit: usize) -> Result<Arc<Field>, String> {
    use image::ImageDecoder;
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(limit as u64);
    reader.limits(limits);
    let decoder = reader.into_decoder().map_err(|e| {
        if matches!(e, image::ImageError::Limits(_)) {
            format!("DEM admission: {e}")
        } else {
            format!("Undecodable DEM image: {e}")
        }
    })?;
    let dimensions = decoder.dimensions();
    let count = (dimensions.0 as usize)
        .checked_mul(dimensions.1 as usize)
        .ok_or("DEM image dimensions overflow")?;
    // Peak is original decoded pixels+RGBA conversion, or RGBA+f32 result.
    let pixel_peak = (usize::from(decoder.color_type().bytes_per_pixel()) + 4).max(8);
    let peak = count
        .checked_mul(pixel_peak)
        .ok_or("DEM image peak overflow")?;
    if count == 0 || peak > limit {
        return Err("DEM decoded tile exceeds admitted CPU working budget".into());
    }
    let rgba = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("Undecodable DEM image: {e}"))?
        .into_rgba8();
    let field = Field::from_rgba(
        dimensions.0,
        dimensions.1,
        rgba.as_raw(),
        encoding,
        limit.saturating_sub(rgba.as_raw().capacity()),
    )?;
    Ok(Arc::new(field))
}

fn tile(agent: &ureq::Agent, job: &Job) -> Result<Ready, String> {
    let cached = job.cache.lock().unwrap_or_else(|e| e.into_inner()).get(
        &job.source,
        job.tile,
        job.max_bytes,
    )?;
    if let Some(bytes) = cached {
        match decode(&bytes, job.source.encoding, job.decoded_limit) {
            Ok(field) => {
                return Ok(Ready {
                    field,
                    cache_hit: true,
                    encoded_bytes: bytes.len(),
                })
            }
            Err(e) if admission_error(&e) => return Err(e),
            Err(_) => {}
        }
    }
    // Corrupt cache entries fall through to actual transport, as TileFetcher.
    let bytes = fetch(agent, job)?;
    let field = decode(&bytes, job.source.encoding, job.decoded_limit)?;
    job.cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .put(&job.source, job.tile, &bytes)?;
    Ok(Ready {
        field,
        cache_hit: false,
        encoded_bytes: bytes.len(),
    })
}
fn fetch(agent: &ureq::Agent, job: &Job) -> Result<Vec<u8>, String> {
    let mut url = url::Url::parse(&job.url).map_err(|e| e.to_string())?;
    let mut redirects = 0usize;
    let started = std::time::Instant::now();
    let response = loop {
        let response = agent
            .get(url.as_str())
            .timeout(
                job.deadline
                    .checked_sub(started.elapsed())
                    .ok_or("DEM transfer deadline exceeded")?,
            )
            .set(
                "User-Agent",
                concat!(
                    "IngeTrazo/",
                    env!("CARGO_PKG_VERSION"),
                    " (https://github.com/ingelibre/ingetrazo)"
                ),
            )
            .call()
            .map_err(|e| format!("DEM tile transport: {e}"))?;
        if matches!(response.status(), 301 | 302 | 303 | 307 | 308) {
            if redirects >= 50 {
                return Err("DEM redirect limit exceeded".into());
            }
            let location = response
                .header("Location")
                .ok_or("DEM redirect has no destination")?;
            let next = url.join(location).map_err(|e| e.to_string())?;
            if !matches!(next.scheme(), "http" | "https")
                || (url.scheme() == "https" && next.scheme() != "https")
            {
                return Err("DEM redirect violates source no-less-safe policy".into());
            }
            url = next;
            redirects += 1;
            continue;
        }
        break response;
    };
    if response
        .header("Content-Length")
        .and_then(|v| v.parse::<usize>().ok())
        .is_some_and(|n| n > job.max_bytes)
    {
        return Err("DEM response exceeds admitted encoded-byte budget".into());
    }
    let mut body = response.into_reader();
    let mut out = Vec::new();
    let mut scratch = [0u8; 8192];
    loop {
        let read = body
            .read(&mut scratch)
            .map_err(|e| format!("DEM response: {e}"))?;
        if read == 0 {
            break;
        }
        let need = out
            .len()
            .checked_add(read)
            .ok_or("DEM response length overflow")?;
        if need > job.max_bytes {
            return Err("DEM response exceeds admitted encoded-byte budget".into());
        }
        if need > out.capacity() {
            let capacity = need.max(out.capacity().saturating_mul(2).min(job.max_bytes));
            out.try_reserve_exact(capacity - out.len())
                .map_err(|_| "DEM response allocation refused")?;
        }
        if out.capacity() > job.max_bytes {
            return Err("DEM response capacity exceeds admitted encoded-byte budget".into());
        }
        out.extend_from_slice(&scratch[..read]);
    }
    if out.is_empty() {
        return Err("Undecodable empty DEM tile".into());
    }
    Ok(out)
}

/// Source disk LRU. The ordered timestamp index is built once; put looks at
/// at most two maximum entries to exclude an overwritten newest file.
pub struct DiskCache {
    root: std::path::PathBuf,
    max_bytes: u64,
    index_budget: usize,
    files: std::collections::HashMap<std::path::PathBuf, (u64, std::time::SystemTime)>,
    ordered: std::collections::BTreeSet<(std::time::SystemTime, std::path::PathBuf)>,
    total: u64,
    index_charge: usize,
    indexed: bool,
}
impl DiskCache {
    pub fn new(root: std::path::PathBuf, max_bytes: u64, index_budget: usize) -> Self {
        Self {
            root,
            max_bytes,
            index_budget,
            files: Default::default(),
            ordered: Default::default(),
            total: 0,
            index_charge: 0,
            indexed: false,
        }
    }
    pub fn reserved_index_bytes(&self) -> usize {
        self.index_budget
    }
    pub fn path(&self, source: &Source, t: Tile) -> std::path::PathBuf {
        self.root
            .join(&source.id)
            .join(t.zoom.to_string())
            .join(t.x.to_string())
            .join(t.y.to_string())
    }
    // Bookkeeping is bounded by admitted entries/path capacity. Conservative
    // control-plane allowance, not a claim about the entire allocator/process.
    fn charge(path: &std::path::PathBuf) -> Result<usize, String> {
        path.capacity()
            .checked_mul(4)
            .and_then(|n| n.checked_add(512))
            .ok_or_else(|| "DEM cache index charge overflow".into())
    }
    fn index(&mut self) -> Result<(), String> {
        if self.indexed {
            return Ok(());
        }
        std::fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let mut dirs = vec![self.root.clone()];
        let mut pending_directory_charge = Self::charge(&dirs[0])?;
        if pending_directory_charge > self.index_budget {
            return Err("DEM cache root exceeds index budget".into());
        }
        let mut files = std::collections::HashMap::new();
        let mut ordered = std::collections::BTreeSet::new();
        let mut total = 0u64;
        let mut charge = 0usize;
        while let Some(dir) = dirs.pop() {
            pending_directory_charge = pending_directory_charge.saturating_sub(Self::charge(&dir)?);
            for item in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
                let item = item.map_err(|e| e.to_string())?;
                let meta = item.metadata().map_err(|e| e.to_string())?;
                let path = item.path();
                if meta.is_dir() {
                    let pending = pending_directory_charge
                        .checked_add(Self::charge(&path)?)
                        .ok_or("DEM directory charge overflow")?;
                    if charge.checked_add(pending).ok_or("DEM index overflow")? > self.index_budget
                    {
                        return Err(
                            "DEM cache directory roster exceeds configured CPU budget".into()
                        );
                    }
                    pending_directory_charge = pending;
                    dirs.try_reserve(1)
                        .map_err(|_| "DEM cache directory allocation refused")?;
                    dirs.push(path);
                } else if meta.is_file() {
                    charge = charge
                        .checked_add(Self::charge(&path)?)
                        .ok_or("DEM index overflow")?;
                    if charge
                        .checked_add(pending_directory_charge)
                        .ok_or("DEM combined index charge overflow")?
                        > self.index_budget
                    {
                        return Err("DEM disk-cache index exceeds configured CPU budget".into());
                    }
                    files
                        .try_reserve(1)
                        .map_err(|_| "DEM cache index allocation refused")?;
                    let stamp = meta.modified().map_err(|e| e.to_string())?;
                    total = total
                        .checked_add(meta.len())
                        .ok_or("DEM cache total overflow")?;
                    ordered.insert((stamp, path.clone()));
                    files.insert(path, (meta.len(), stamp));
                }
            }
        }
        self.files = files;
        self.ordered = ordered;
        self.total = total;
        self.index_charge = charge;
        self.indexed = true;
        Ok(())
    }
    pub fn get(
        &mut self,
        source: &Source,
        t: Tile,
        max_encoded: usize,
    ) -> Result<Option<Vec<u8>>, String> {
        self.index()?;
        let path = self.path(source, t);
        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => return Ok(None),
        };
        if meta.len() > max_encoded as u64 {
            return Err("DEM cached tile exceeds admitted encoded-byte budget".into());
        }
        let mut file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };
        let mut data = Vec::new();
        let mut scratch = [0u8; 8192];
        loop {
            let read = match file.read(&mut scratch) {
                Ok(n) => n,
                Err(_) => return Ok(None),
            };
            if read == 0 {
                break;
            }
            let need = data
                .len()
                .checked_add(read)
                .ok_or("DEM cached size overflow")?;
            if need > max_encoded {
                return Err("DEM cached tile grew beyond admitted budget".into());
            }
            if need > data.capacity() {
                let capacity = need.max(data.capacity().saturating_mul(2).min(max_encoded));
                data.try_reserve_exact(capacity - data.len())
                    .map_err(|_| "DEM cached allocation refused")?;
            }
            if data.capacity() > max_encoded {
                return Err("DEM cached capacity exceeds admitted budget".into());
            }
            data.extend_from_slice(&scratch[..read]);
        }
        // Source touch is best effort; do not alter LRU order if it fails.
        if let Ok(old) = meta.modified() {
            if let Some(new) = old.checked_add(Duration::from_secs(1)) {
                if std::fs::File::options()
                    .write(true)
                    .open(&path)
                    .and_then(|metadata| {
                        metadata.set_times(std::fs::FileTimes::new().set_modified(new))
                    })
                    .is_ok()
                {
                    if let Some(info) = self.files.get_mut(&path) {
                        self.ordered.remove(&(info.1, path.clone()));
                        info.1 = new;
                        self.ordered.insert((new, path));
                    }
                }
            }
        }
        Ok(Some(data))
    }
    pub fn put(&mut self, source: &Source, t: Tile, data: &[u8]) -> Result<(), String> {
        if data.is_empty() {
            return Ok(());
        }
        self.index()?;
        let path = self.path(source, t);
        let old = self.files.get(&path).copied();
        let charge = if old.is_some() {
            self.index_charge
        } else {
            self.index_charge
                .checked_add(Self::charge(&path)?)
                .ok_or("DEM cache index overflow")?
        };
        if charge > self.index_budget {
            return Err("DEM cache index admission exceeds configured CPU budget".into());
        }
        self.files
            .try_reserve(1)
            .map_err(|_| "DEM cache index allocation refused")?;
        let mut maxima = self.ordered.iter().rev();
        let newest = maxima
            .next()
            .filter(|(_, p)| *p != path)
            .or_else(|| maxima.next())
            .map(|v| v.0);
        let now = std::time::SystemTime::now();
        let stamp = match newest {
            Some(newest) if now <= newest => newest
                .checked_add(Duration::from_secs(1))
                .ok_or("DEM cache timestamp overflow")?,
            _ => now,
        };
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let temp = path.with_extension(format!("pending-{}", uuid::Uuid::new_v4()));
        let write = (|| {
            let mut file = std::fs::File::create(&temp).map_err(|e| e.to_string())?;
            std::io::Write::write_all(&mut file, data).map_err(|e| e.to_string())?;
            file.set_times(std::fs::FileTimes::new().set_modified(stamp))
                .map_err(|e| e.to_string())?;
            std::fs::rename(&temp, &path).map_err(|e| e.to_string())
        })();
        if write.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        write?;
        if let Some((bytes, time)) = old {
            self.total = self.total.saturating_sub(bytes);
            self.ordered.remove(&(time, path.clone()));
        }
        self.total = self
            .total
            .checked_add(data.len() as u64)
            .ok_or("DEM cache total overflow")?;
        self.index_charge = charge;
        self.files.insert(path.clone(), (data.len() as u64, stamp));
        self.ordered.insert((stamp, path));
        // Eviction uses one moving cursor; no full-cache cloned roster allocation.
        let mut previous = None;
        while self.total > self.max_bytes {
            use std::ops::Bound::{Excluded, Unbounded};
            let next = match &previous {
                None => self.ordered.first().cloned(),
                Some(p) => self.ordered.range((Excluded(p), Unbounded)).next().cloned(),
            };
            let Some((stamp, path)) = next else {
                break;
            };
            if std::fs::remove_file(&path).is_ok() {
                if let Some((bytes, _)) = self.files.remove(&path) {
                    self.total = self.total.saturating_sub(bytes);
                    self.index_charge = self.index_charge.saturating_sub(Self::charge(&path)?);
                }
                self.ordered.remove(&(stamp, path.clone()));
            }
            previous = Some((stamp, path));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn bounded_body_deadline_and_cache_lru() {
        let root = std::env::temp_dir().join(format!("dem-cache-{}", uuid::Uuid::new_v4()));
        let source = Source::default();
        let a = Tile {
            x: 1,
            y: 2,
            zoom: 3,
        };
        let b = Tile {
            x: 2,
            y: 2,
            zoom: 3,
        };
        let mut cache = DiskCache::new(root.clone(), 3, 64 * 1024);
        cache.put(&source, a, b"abc").unwrap();
        cache.put(&source, b, b"de").unwrap();
        assert!(cache.get(&source, a, 3).unwrap().is_none());
        assert_eq!(cache.get(&source, b, 3).unwrap().unwrap(), b"de");
        cache.put(&source, b, b"xyz").unwrap();
        assert_eq!(cache.get(&source, b, 3).unwrap().unwrap(), b"xyz");
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = server.local_addr().unwrap();
        let thread = std::thread::spawn(move || {
            let (mut socket, _) = server.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let mut request = [0; 4096];
            let _ = socket.read(&mut request);
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\n")
                .unwrap();
            std::thread::sleep(Duration::from_millis(250));
        });
        let job = Job {
            tile: a,
            generation: 1,
            url: format!("http://{address}/tile"),
            max_bytes: 8,
            deadline: Duration::from_millis(50),
            source,
            cache: Arc::new(Mutex::new(cache)),
            decoded_limit: 1024,
        };
        let begin = std::time::Instant::now();
        assert!(fetch(&ureq::AgentBuilder::new().redirects(0).build(), &job).is_err());
        assert!(begin.elapsed() < Duration::from_secs(1));
        thread.join().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
