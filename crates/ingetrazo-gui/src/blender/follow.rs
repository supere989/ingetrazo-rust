use super::*;
#[derive(Default)]
pub struct FollowState {
    pub enabled: bool,
    seen: Option<String>,
    since: f64,
    submitted: Option<(String, String)>,
}
impl FollowState {
    pub(super) fn retry_after_failure(&mut self) {
        self.submitted = None;
    }
    fn due(&mut self, now: f64, buttons: bool, camera: &str, scene: &str, busy: bool) -> bool {
        if !self.enabled {
            return false;
        }
        if self.seen.as_deref() != Some(camera) {
            self.seen = Some(camera.into());
            self.since = now;
            return false;
        }
        if buttons {
            self.since = now;
            return false;
        }
        if now - self.since < 0.7 || busy {
            return false;
        }
        if self
            .submitted
            .as_ref()
            .is_some_and(|(c, s)| c == camera && s == scene)
        {
            return false;
        }
        self.submitted = Some((camera.into(), scene.into()));
        true
    }
}
fn round(v: f64, n: usize) -> f64 {
    format!("{v:.n$}").parse().unwrap_or(v)
}
pub fn camera_key(camera: &OrbitCamera) -> String {
    let d = ingetrazo_core::render_blender::camera_dict(camera);
    let xyz = ["eye", "target", "up"]
        .into_iter()
        .flat_map(|k| {
            d[k].as_array()
                .unwrap()
                .iter()
                .map(|v| round(v.as_f64().unwrap_or(0.), 3))
        })
        .collect::<Vec<_>>();
    serde_json::json!([
        xyz,
        d["perspective"],
        round(camera.fov_deg, 2),
        round(d["half_height"].as_f64().unwrap_or(0.), 3)
    ])
    .to_string()
}
impl BlenderRenderState {
    pub fn set_follow(&mut self, enabled: bool) {
        if self.follow.enabled == enabled {
            return;
        }
        self.follow = FollowState {
            enabled,
            ..Default::default()
        };
        if !enabled {
            self.worker = None;
            self.rx = None;
            self.process_id = None;
            self.reuse_key = None;
            self.is_rendering = false;
        }
    }
    pub fn tick_follow(
        &mut self,
        scene: &Scene,
        camera: &OrbitCamera,
        revision: u64,
        now: f64,
        buttons: bool,
    ) {
        if !self.follow.enabled {
            return;
        }
        let camera_key = camera_key(camera);
        let scene_key = serde_json::json!([
            revision,
            scene.version,
            scene.shadows,
            scene.geo_datum,
            scene.display_style,
            self.engine,
            self.ambience,
            self.ground,
            self.sun_scale,
            self.lights,
            round(camera.aspect, 3)
        ])
        .to_string();
        if !self
            .follow
            .due(now, buttons, &camera_key, &scene_key, self.is_rendering)
        {
            return;
        }
        let height = (960. / camera.aspect).round_ties_even().clamp(2., 16384.) as u32;
        let samples = if self.engine == BlenderEngine::Cycles {
            16
        } else {
            8
        };
        if let Err(e) =
            self.prepare_render(scene, camera, Some(revision), Some((960, height, samples)))
        {
            self.follow.enabled = false;
            self.status_line = format!("Error: {e}");
        }
    }
}
impl IngeTrazoApp {
    pub fn tick_blender_follow(&mut self, ctx: &egui::Context) {
        if self.blender_state.follow.enabled && !self.blender_state.pending_document_controls {
            let (now, buttons) = ctx.input(|i| (i.time, i.pointer.any_down()));
            if let Err(error) = self.prepare_blender_projection() {
                self.blender_state.follow.enabled = false;
                self.blender_state.status_line = format!("Error: {error}");
                return;
            }
            self.with_blender_projection(|state, scene, camera, revision| {
                state.tick_follow(scene, camera, revision, now, buttons);
            });
            ctx.request_repaint_after(std::time::Duration::from_millis(150));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rest_busy_latest_camera_and_document_invalidation() {
        let mut f = FollowState {
            enabled: true,
            ..Default::default()
        };
        assert!(!f.due(0., false, "A", "scene", false));
        assert!(!f.due(0.8, true, "A", "scene", false));
        assert!(!f.due(1.49, false, "A", "scene", false));
        assert!(f.due(1.51, false, "A", "scene", false));
        assert!(!f.due(2., false, "B", "scene", true));
        assert!(!f.due(2.8, false, "B", "scene", true));
        assert!(!f.due(3., false, "C", "scene", true));
        assert!(!f.due(3.8, false, "C", "scene", true));
        assert!(f.due(4., false, "C", "scene", false));
        assert!(!f.due(5., false, "C", "scene", false));
        assert!(f.due(6., false, "C", "changed", false));
    }
}
