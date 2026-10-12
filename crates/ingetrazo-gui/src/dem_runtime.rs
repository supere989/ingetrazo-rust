//! The Profile-only DEM owner. Cold explicit actions can serve disk-cache hits
//! synchronously; all background cache/network/decode work is worker-owned.
use crate::dem_tiles::{
    admission_error, decode, DiskCache, Source, Transport, CONTROL_RESERVATION,
};
use glam::DVec3;
use ingetrazo_core::georef::SceneDatum;
use ingetrazo_formats::dem_profile::{query, Area, Field, Tile};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
pub struct Service {
    pub cache: Arc<Mutex<DiskCache>>,
    pool: OnceLock<Mutex<Transport>>,
    encoded: usize,
    decoded: usize,
    index: usize,
    pub deadline: Duration,
}
impl Service {
    pub fn new(
        root: std::path::PathBuf,
        budget: usize,
        deadline: Duration,
    ) -> Result<Arc<Self>, String> {
        let encoded = (budget / 16).min(16 * 1024 * 1024);
        let decoded = (budget / 16).min(16 * 1024 * 1024);
        let index = (budget / 32).min(8 * 1024 * 1024);
        if encoded <= CONTROL_RESERVATION + 17 || decoded < 17 || index == 0 {
            return Err("DEM service admission exceeds configured CPU budget".into());
        }
        Ok(Arc::new(Self {
            cache: Arc::new(Mutex::new(DiskCache::new(root, 256 * 1024 * 1024, index))),
            pool: OnceLock::new(),
            encoded,
            decoded,
            index,
            deadline,
        }))
    }
    pub fn resident_reservation(&self) -> usize {
        self.encoded + self.decoded + self.index
    }
    pub fn decoded_limit(&self) -> usize {
        self.decoded / 17
    }
    pub fn encoded_limit(&self) -> usize {
        (self.encoded - CONTROL_RESERVATION) / 17
    }
    fn pool(&self) -> Result<&Mutex<Transport>, String> {
        if let Some(pool) = self.pool.get() {
            return Ok(pool);
        }
        let pool = Mutex::new(Transport::new(self.deadline, self.encoded, self.decoded)?);
        let _ = self.pool.set(pool);
        Ok(self.pool.get().unwrap())
    }
    pub fn set_wake(&self, ctx: &egui::Context) {
        if let Some(p) = self.pool.get() {
            p.lock().unwrap_or_else(|e| e.into_inner()).set_wake(ctx);
        }
    }
    pub fn retire(&self) {
        if let Some(pool) = self.pool.get() {
            pool.lock()
                .unwrap_or_else(|e| e.into_inner())
                .cancel_generation();
        }
    }
    pub fn pending(&self) -> usize {
        self.pool
            .get()
            .map(|p| p.lock().unwrap_or_else(|e| e.into_inner()).pending.len())
            .unwrap_or(0)
    }
}
pub struct Cache {
    pub service: Arc<Service>,
    pub source: Source,
    pub zoom: u8,
    fields: HashMap<Tile, Arc<Field>>,
    field_bytes: usize,
    pub epoch: u64,
    pub decoded_count: u64,
    area: Option<Area>,
    area_key: Option<[i32; 5]>,
    wanted: VecDeque<Tile>,
    wanted_set: HashSet<Tile>,
    cold_misses: HashSet<Tile>,
    pub generation: u64,
    pub last_error: Option<String>,
    pub requested: u64,
}
impl Cache {
    pub fn new(service: Arc<Service>, source: Source, zoom: u8) -> Result<Self, String> {
        source.validate()?;
        if zoom > source.max_zoom {
            return Err("DEM source zoom is out of range".into());
        }
        let generation = service
            .pool
            .get()
            .map(|p| p.lock().unwrap_or_else(|e| e.into_inner()).generation())
            .unwrap_or(1);
        Ok(Self {
            service,
            source,
            zoom,
            fields: Default::default(),
            field_bytes: 0,
            epoch: 0,
            decoded_count: 0,
            area: None,
            area_key: None,
            wanted: Default::default(),
            wanted_set: Default::default(),
            cold_misses: Default::default(),
            generation,
            last_error: None,
            requested: 0,
        })
    }
    // Explicit actions stage only metadata; immutable decoded pixels remain shared.
    pub fn fork(&self, available: usize) -> Result<Self, String> {
        let estimate = self
            .fields
            .len()
            .checked_add(self.wanted.len() * 2)
            .and_then(|n| n.checked_mul(256))
            .ok_or("DEM metadata overflow")?;
        if estimate > available {
            return Err("DEM staged metadata exceeds CPU budget".into());
        }
        let mut c = Self::new(self.service.clone(), self.source.clone(), self.zoom)?;
        c.fields
            .try_reserve(self.fields.len())
            .map_err(|_| "DEM staged field index refused")?;
        c.fields
            .extend(self.fields.iter().map(|(t, f)| (*t, f.clone())));
        c.field_bytes = self.field_bytes;
        c.epoch = self.epoch;
        c.decoded_count = self.decoded_count;
        c.area = self.area.clone();
        c.area_key = self.area_key;
        c.generation = self.generation;
        c.wanted
            .try_reserve(self.wanted.len())
            .map_err(|_| "DEM staged queue refused")?;
        c.wanted.extend(self.wanted.iter().copied());
        c.wanted_set
            .try_reserve(self.wanted_set.len())
            .map_err(|_| "DEM staged queue index refused")?;
        c.wanted_set.extend(self.wanted_set.iter().copied());
        c.last_error = self.last_error.clone();
        c.requested = self.requested;
        c.reserve_publication_queues()?;
        if c.field_bytes().saturating_sub(c.field_data_bytes()) > available {
            return Err("DEM staged capacities exceed CPU budget".into());
        }
        Ok(c)
    }
    pub fn field_data_bytes(&self) -> usize {
        self.field_bytes
    }
    pub fn reserve_publication_queues(&mut self) -> Result<(), String> {
        self.wanted
            .try_reserve(257 - self.wanted.len())
            .map_err(|_| "DEM publication queue admission refused")?;
        self.wanted_set
            .try_reserve(257 - self.wanted_set.len())
            .map_err(|_| "DEM publication index admission refused")?;
        self.cold_misses
            .try_reserve(257)
            .map_err(|_| "DEM cold index admission refused")?;
        Ok(())
    }
    // All bounded buffers/worker ownership were admitted before this commit.
    // A transport refusal is recorded as a gap, never a partial Profile refusal.
    pub fn enqueue_prepared(&mut self) {
        if let Err(e) = self.pump() {
            self.last_error = Some(e);
        }
    }
    pub fn admit_transport(&self) -> Result<(), String> {
        if self.needs_pump() {
            let _ = self.service.pool()?;
        }
        Ok(())
    }
    pub fn field_bytes(&self) -> usize {
        self.field_bytes
            + self.fields.capacity() * 96
            + self.wanted.capacity() * 16
            + self.wanted_set.capacity() * 32
            + self.cold_misses.capacity() * 32
    }
    pub fn arrived(&self) -> Vec<Tile> {
        let mut t: Vec<_> = self.fields.keys().copied().collect();
        t.sort_by_key(|t| (t.x, t.y, t.zoom));
        t
    }
    fn accept(&mut self, tile: Tile, field: Arc<Field>, data_limit: usize) -> Result<(), String> {
        let old = self
            .fields
            .get(&tile)
            .map(|f| f.resident_bytes())
            .unwrap_or(0);
        let needed = self
            .field_bytes
            .saturating_sub(old)
            .checked_add(field.resident_bytes())
            .ok_or("DEM cache field bytes overflow")?;
        if needed > data_limit {
            return Err("DEM cached fields exceed configured profile CPU budget".into());
        }
        if needed
            .checked_add((self.fields.len() + 1) * 192)
            .is_none_or(|n| n > data_limit)
        {
            return Err("DEM field index exceeds CPU budget".into());
        }
        self.fields
            .try_reserve(1)
            .map_err(|_| "DEM field index allocation refused")?;
        if needed
            .checked_add(self.fields.capacity() * 96)
            .is_none_or(|n| n > data_limit)
        {
            return Err("DEM field capacity exceeds CPU budget".into());
        }
        self.field_bytes = needed;
        self.fields.insert(tile, field);
        self.epoch = self.epoch.wrapping_add(1);
        self.decoded_count += 1;
        Ok(())
    }
    pub fn begin_cold_action(&mut self) {
        self.cold_misses.clear();
    }
    pub fn ensure_area(&mut self, datum: &SceneDatum, points: &[DVec3]) -> Result<(), String> {
        let area = Area::for_path(datum, points, self.zoom)?;
        let key = area.key();
        if self.area_key != Some(key) {
            self.area_key = Some(key);
            self.area = Some(area);
        }
        Ok(())
    }
    fn wanted(&mut self, tile: Tile) -> Result<(), String> {
        if self.fields.contains_key(&tile) || self.wanted_set.contains(&tile) {
            return Ok(());
        }
        if self.wanted.len() >= 257 {
            return Err("DEM station queue exceeds bounded request capacity".into());
        }
        self.wanted
            .try_reserve(1)
            .map_err(|_| "DEM pending tile allocation refused")?;
        self.wanted_set
            .try_reserve(1)
            .map_err(|_| "DEM pending tile index refused")?;
        self.wanted.push_back(tile);
        self.wanted_set.insert(tile);
        Ok(())
    }
    pub fn sample(
        &mut self,
        datum: &SceneDatum,
        point: DVec3,
        data_limit: usize,
        cold: bool,
    ) -> Result<Option<f64>, String> {
        let q = query(datum, point, self.zoom)?;
        if !self.fields.contains_key(&q.tile) && cold && !self.cold_misses.contains(&q.tile) {
            self.cold_misses
                .try_reserve(1)
                .map_err(|_| "DEM cold tile index refused")?;
            self.cold_misses.insert(q.tile);
            let data = self
                .service
                .cache
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&self.source, q.tile, self.service.encoded_limit())?;
            if let Some(bytes) = data {
                match decode(&bytes, self.source.encoding, self.service.decoded_limit()) {
                    Ok(field) => self.accept(q.tile, field, data_limit)?,
                    Err(e) if admission_error(&e) => return Err(e),
                    Err(_) => {}
                }
            }
        }
        if let Some(field) = self.fields.get(&q.tile) {
            return field.bilinear(q.pixel[0], q.pixel[1]).map(Some);
        }
        self.wanted(q.tile)?;
        Ok(None)
    }
    /// Pump only a bounded batch; the Area cursor retains the whole requested bbox.
    pub fn pump(&mut self) -> Result<(), String> {
        if self.wanted.is_empty() && self.area.as_ref().is_none_or(|a| a.remaining() == 0) {
            return Ok(());
        }
        while self.wanted.len() < 257 {
            let Some(tile) = self.area.as_mut().and_then(Area::next_tile) else {
                break;
            };
            if !self.fields.contains_key(&tile)
                && !self.service.pool.get().is_some_and(|p| {
                    p.lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .pending
                        .contains(&tile)
                })
                && !self.wanted_set.contains(&tile)
            {
                self.wanted
                    .try_reserve(1)
                    .map_err(|_| "DEM area queue allocation refused")?;
                self.wanted_set
                    .try_reserve(1)
                    .map_err(|_| "DEM area index refused")?;
                self.wanted.push_back(tile);
                self.wanted_set.insert(tile);
            }
        }
        if self.wanted.is_empty() {
            return Ok(());
        }
        let pool = self.service.pool()?;
        let mut pool = pool.lock().unwrap_or_else(|e| e.into_inner());
        self.generation = pool.generation();
        while let Some(tile) = self.wanted.front().copied() {
            match pool.request(
                &self.source,
                tile,
                self.service.encoded_limit(),
                self.service.cache.clone(),
            ) {
                Ok(new) => {
                    self.wanted.pop_front();
                    self.wanted_set.remove(&tile);
                    self.requested += u64::from(new);
                }
                Err(e) if e.contains("queue admission") => break,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
    pub fn needs_pump(&self) -> bool {
        !self.wanted.is_empty() || self.area.as_ref().is_some_and(|a| a.remaining() > 0)
    }
    pub fn poll_one(&mut self, data_limit: usize) -> Option<Result<bool, String>> {
        let result = self
            .service
            .pool
            .get()?
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .poll()?;
        if result.generation != self.generation {
            return Some(Ok(false));
        }
        Some(match result.result {
            Ok(ready) => self
                .accept(result.tile, ready.field, data_limit)
                .map(|_| true),
            Err(e) => {
                self.last_error = Some(e);
                Ok(false)
            }
        })
    }
}
