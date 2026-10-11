//! Three-click source Change Axes interaction. Hover never publishes geometry.
use crate::base::{Tool, ToolContext, ToolType};
use glam::{DMat4, DVec3, Vec3};
use ingetrazo_core::{change_axes::Command, scene::Scene};
use uuid::Uuid;
#[derive(Default)]
pub struct ChangeAxesTool {
    target: Option<Uuid>,
    origin: Option<Vec3>,
    red: Option<Vec3>,
    hover: Option<Vec3>,
    done: bool,
    status: Option<String>,
    error: Option<String>,
    command: Option<Command>,
}
fn length(p: Vec3) -> f32 {
    let a = p.abs();
    let max = a.x.max(a.y).max(a.z);
    if max == 0. {
        0.
    } else {
        let a = a / max;
        max * (a.x * a.x + a.y * a.y + a.z * a.z).sqrt()
    }
}
fn normalized(p: Vec3) -> Vec3 {
    let n = length(p);
    if (n - 1.).abs() <= 0.00001 {
        p
    } else if n.abs() <= 0.00001 {
        Vec3::ZERO
    } else {
        p / n
    }
}
impl ChangeAxesTool {
    pub fn new(target: Uuid) -> Self {
        Self {
            target: Some(target),
            ..Default::default()
        }
    }
    fn axes(&self, green: Vec3) -> Option<[Vec3; 3]> {
        let origin = self.origin?;
        let x = self.red? - origin;
        if length(x) < 1e-9 {
            return None;
        }
        let x = normalized(x);
        let g = green - origin;
        let y = g - x * g.dot(x);
        if length(y) < 1e-6 {
            return None;
        }
        let y = normalized(y);
        Some([x, y, normalized(x.cross(y))])
    }
    fn frame(&self, p: Vec3) -> Option<DMat4> {
        let [x, y, z] = self.axes(p)?;
        Some(DMat4::from_cols(
            x.as_dvec3().extend(0.),
            y.as_dvec3().extend(0.),
            z.as_dvec3().extend(0.),
            self.origin?.as_dvec3().extend(1.),
        ))
    }
}
impl Tool for ChangeAxesTool {
    fn tool_type(&self) -> ToolType {
        ToolType::ChangeAxes
    }
    fn name(&self) -> &'static str {
        "Change Axes"
    }
    fn on_activate(&mut self, _: &mut Scene) {
        self.origin = None;
        self.red = None;
        if self.target.is_none() {
            self.status = Some("Select one group or component first".into())
        }
    }
    fn on_move(&mut self, _: &mut Scene, c: &ToolContext) {
        if c.world.as_vec3().is_finite() {
            self.hover = Some(c.world.as_vec3())
        }
    }
    fn on_click(&mut self, s: &mut Scene, c: &ToolContext) {
        self.error = None;
        let Some(target) = self.target else { return };
        let p = c.world.as_vec3();
        if !p.is_finite() {
            self.error = Some("Axes point must be finite".into());
            return;
        }
        if self.origin.is_none() {
            self.origin = Some(p);
            self.status = Some("Click along the new red axis".into());
            return;
        }
        if self.red.is_none() {
            if length(p - self.origin.unwrap()) < 1e-6 {
                return;
            }
            self.red = Some(p);
            self.status = Some("Click to set the green direction".into());
            return;
        }
        let Some(frame) = self.frame(p) else {
            self.status =
                Some("The green direction cannot run along red — click off the red axis".into());
            return;
        };
        match Command::prepare(s, target, frame).and_then(|command| {
            command.apply(s)?;
            Ok(command)
        }) {
            Ok(command) => {
                self.command = Some(command);
                self.done = true;
                self.target = None;
                self.status = Some("Axes changed".into())
            }
            Err(e) => self.error = Some(e),
        }
    }
    fn on_cancel(&mut self, _: &mut Scene) {
        self.origin = None;
        self.red = None;
        self.done = true;
    }
    fn completed(&self) -> bool {
        self.done
    }
    fn take_status_message(&mut self) -> Option<String> {
        self.status.take()
    }
    fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }
    fn axes_commit_refused(&mut self, target: Uuid) {
        self.target = Some(target);
        self.done = false;
        self.command = None;
    }
    fn take_axes_command(&mut self) -> Option<Command> {
        self.command.take()
    }
    fn value_label(&self) -> Option<(String, DVec3)> {
        Some((
            if self.red.is_some() {
                "Green axis"
            } else {
                "Red axis"
            }
            .into(),
            self.hover?.as_dvec3(),
        ))
    }
    fn rubber_band_lines(&self) -> Vec<(DVec3, DVec3)> {
        let (Some(o), Some(h)) = (self.origin, self.hover) else {
            return vec![];
        };
        if self.red.is_none() {
            return vec![(o.as_dvec3(), h.as_dvec3())];
        }
        let Some(axes) = self.axes(h) else {
            return vec![(o.as_dvec3(), self.red.unwrap().as_dvec3())];
        };
        let len = length(self.red.unwrap() - o).max(0.5);
        axes.into_iter()
            .map(|a| (o.as_dvec3(), (o + a * len).as_dvec3()))
            .collect()
    }
}
