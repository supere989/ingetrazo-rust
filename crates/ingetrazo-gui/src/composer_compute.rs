// SPDX-License-Identifier: GPL-3.0-or-later
//! Application-owned persistent composer computation and bounded completed-ink cache.
use ingetrazo_core::{camera::OrbitCamera, scene::Scene};
use ingetrazo_render::{
    process_hlr::{HlrProcessPool, ProcessMetrics},
    vector_hlr::{
        original_qt_paper_drawing_with_pool, parallel_segments_with_pool, OriginalQtPaperBasis,
        VectorSegment,
    },
};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::Arc,
};
/// Cache identity includes the geometry/projection publication policy. Worker wire
/// protocol remains f64 projected intervals; geometry from different policies can
/// never reuse completed ink, even with otherwise identical document/camera keys.
#[derive(Clone, Copy, PartialEq, Eq)]
enum InkPolicy {
    NativeF64,
    OriginalQtPaperV1,
}
#[derive(Clone, PartialEq, Eq)]
struct Key {
    policy: InkPolicy,
    document: uuid::Uuid,
    revision: u64,
    scene: u64,
    view: u64,
    camera: [u64; 16],
    profiles: bool,
    hidden: bool,
    view_key: String,
}
pub struct PaperDrawing {
    pub segments: Arc<Vec<VectorSegment>>,
    pub section_material: Vec<[glam::DVec3; 3]>,
}
impl PaperDrawing {
    fn bytes(&self) -> usize {
        self.segments
            .capacity()
            .saturating_mul(std::mem::size_of::<VectorSegment>())
            .saturating_add(
                self.section_material
                    .capacity()
                    .saturating_mul(std::mem::size_of::<[glam::DVec3; 3]>()),
            )
    }
}
pub struct ComposerCompute {
    pool: Option<HlrProcessPool>,
    failure: Option<String>,
    cache: VecDeque<(Key, Arc<PaperDrawing>)>,
    cache_bytes: usize,
    worker: Option<PathBuf>,
    pub completed_jobs: u64,
}
impl Default for ComposerCompute {
    fn default() -> Self {
        Self {
            pool: None,
            failure: None,
            cache: VecDeque::new(),
            cache_bytes: 0,
            worker: None,
            completed_jobs: 0,
        }
    }
}
impl ComposerCompute {
    pub fn with_worker(path: &Path) -> Self {
        Self {
            worker: Some(path.into()),
            ..Default::default()
        }
    }
    pub fn invalidate(&mut self) {
        self.cache.clear();
        self.cache_bytes = 0;
    }
    pub fn worker_ids(&self) -> Vec<u32> {
        self.pool
            .as_ref()
            .map(|p| p.process_ids())
            .unwrap_or_default()
    }
    pub fn metrics(&self) -> Option<&ProcessMetrics> {
        self.pool.as_ref().map(|p| &p.metrics)
    }
    fn worker_path() -> Result<PathBuf, String> {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let parent = executable.parent().ok_or("No executable directory")?;
        let parent = if parent.file_name().is_some_and(|n| n == "examples") {
            parent.parent().ok_or("No build directory")?
        } else {
            parent
        };
        Ok(parent.join(if cfg!(windows) {
            "ingetrazo-render-worker.exe"
        } else {
            "ingetrazo-render-worker"
        }))
    }
    /// A cache hit performs no process startup, geometry publication or IPC.
    /// Errors stay explicit and do not silently switch to another render pipeline.
    pub fn segments(
        &mut self,
        scene: &Scene,
        camera: &OrbitCamera,
        document: uuid::Uuid,
        revision: u64,
        view_key: &str,
        profiles: bool,
        hidden: bool,
    ) -> Result<Arc<Vec<VectorSegment>>, String> {
        self.segments_policy(
            scene,
            camera,
            document,
            revision,
            view_key,
            profiles,
            hidden,
            InkPolicy::NativeF64,
        )
        .map(|drawing| drawing.segments.clone())
    }
    pub fn original_qt_paper_segments(
        &mut self,
        scene: &Scene,
        camera: &OrbitCamera,
        document: uuid::Uuid,
        revision: u64,
        view_key: &str,
        profiles: bool,
        hidden: bool,
    ) -> Result<Arc<Vec<VectorSegment>>, String> {
        self.segments_policy(
            scene,
            camera,
            document,
            revision,
            view_key,
            profiles,
            hidden,
            InkPolicy::OriginalQtPaperV1,
        )
        .map(|drawing| drawing.segments.clone())
    }
    /// Returns cached cut material with ink; a warm admission check performs no
    /// contour construction, triangulation, projection or worker publication.
    pub fn original_qt_paper_drawing(
        &mut self,
        scene: &Scene,
        camera: &OrbitCamera,
        document: uuid::Uuid,
        revision: u64,
        view_key: &str,
        profiles: bool,
        hidden: bool,
    ) -> Result<Arc<PaperDrawing>, String> {
        self.segments_policy(
            scene,
            camera,
            document,
            revision,
            view_key,
            profiles,
            hidden,
            InkPolicy::OriginalQtPaperV1,
        )
    }
    fn segments_policy(
        &mut self,
        scene: &Scene,
        camera: &OrbitCamera,
        document: uuid::Uuid,
        revision: u64,
        view_key: &str,
        profiles: bool,
        hidden: bool,
        policy: InkPolicy,
    ) -> Result<Arc<PaperDrawing>, String> {
        let key = Key {
            policy,
            document,
            revision,
            scene: scene.version,
            view: scene.view_version,
            camera: match policy {
                InkPolicy::NativeF64 => camera.view_matrix().to_cols_array().map(f64::to_bits),
                InkPolicy::OriginalQtPaperV1 => {
                    let b = OriginalQtPaperBasis::from_camera(camera);
                    let mut key = [0; 16];
                    for (i, v) in [b.eye, b.right, b.up, b.forward].iter().enumerate() {
                        for axis in 0..3 {
                            key[i * 3 + axis] = v[axis].to_bits();
                        }
                    }
                    key
                }
            },
            profiles,
            hidden,
            view_key: view_key.into(),
        };
        if let Some((_, ink)) = self.cache.iter().find(|(k, _)| *k == key) {
            return Ok(ink.clone());
        }
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        if self.pool.is_none() {
            let worker = self
                .worker
                .clone()
                .map(Ok)
                .unwrap_or_else(Self::worker_path)?;
            match HlrProcessPool::new(&worker, 2, 64, 8 * 1024 * 1024) {
                Ok(pool) => self.pool = Some(pool),
                Err(e) => {
                    let error = format!("Drawing computation unavailable: {e}");
                    self.failure = Some(error.clone());
                    return Err(error);
                }
            }
        }
        let result = match policy {
            InkPolicy::NativeF64 => parallel_segments_with_pool(
                scene,
                camera,
                profiles,
                hidden,
                self.pool.as_mut().unwrap(),
            )
            .map(|segments| PaperDrawing {
                segments: Arc::new(segments),
                section_material: Vec::new(),
            }),
            InkPolicy::OriginalQtPaperV1 => original_qt_paper_drawing_with_pool(
                scene,
                camera,
                profiles,
                hidden,
                self.pool.as_mut().unwrap(),
            )
            .map(|drawing| PaperDrawing {
                segments: Arc::new(drawing.segments),
                section_material: drawing.section_material,
            }),
        };
        let ink = match result {
            Ok(ink) => Arc::new(ink),
            Err(e) => {
                let error = format!("Drawing computation failed: {e}");
                self.pool = None;
                self.failure = Some(error.clone());
                return Err(error);
            }
        };
        self.completed_jobs = self.completed_jobs.saturating_add(1);
        let bytes = ink.bytes();
        const BUDGET: usize = 32 * 1024 * 1024;
        while !self.cache.is_empty()
            && (self.cache_bytes.saturating_add(bytes) > BUDGET || self.cache.len() >= 32)
        {
            let (_, old) = self.cache.pop_front().unwrap();
            self.cache_bytes = self.cache_bytes.saturating_sub(old.bytes());
        }
        if bytes <= BUDGET {
            self.cache_bytes += bytes;
            self.cache.push_back((key, ink.clone()));
        }
        Ok(ink)
    }
}
