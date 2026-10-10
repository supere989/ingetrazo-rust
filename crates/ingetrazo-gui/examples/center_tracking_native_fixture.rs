// SPDX-License-Identifier: GPL-3.0-or-later
//! Actual native Vulkan application/input path; writes observational state only.
use glam::DVec3;
use ingetrazo_gui::{
    native_vulkan::{App, System},
    IngeTrazoApp,
};
use ingetrazo_tools::base::ToolType;
struct Fixture {
    app: IngeTrazoApp,
    dir: std::path::PathBuf,
    mode: String,
}
impl App for Fixture {
    fn title() -> &'static str {
        "IngeTrazo native center tracking acceptance"
    }
    fn new(_: &mut System, path: Option<std::path::PathBuf>) -> Self {
        let mut app = IngeTrazoApp::new();
        app.show_tray = false;
        app.native_gpu_viewport_enabled = true;
        let mode = std::env::var("INGETRAZO_CENTER_MODE").unwrap_or("curve".into());
        let radius = if mode == "tiny" { 0.01 } else { 1.2 };
        let center = DVec3::new(2., 3., 1.);
        let ids = (0..24)
            .map(|i| {
                let a = i as f64 * std::f64::consts::TAU / 24.;
                app.scene
                    .mesh
                    .add_vertex(center + DVec3::new(radius * a.cos(), radius * a.sin(), 0.))
            })
            .collect::<Vec<_>>();
        for i in 0..24 {
            let id = app.scene.mesh.add_edge(ids[i], ids[(i + 1) % 24]).unwrap();
            app.scene.mesh.edges[id].curve_id = Some(71);
        }
        if mode == "malformed" {
            let v = app.scene.mesh.add_vertex(center + DVec3::X * 3.);
            let id = app.scene.mesh.add_edge(ids[0], v).unwrap();
            app.scene.mesh.edges[id].curve_id = Some(71);
        }
        app.camera.target = DVec3::new(2., 3., 0.);
        app.camera.distance = 10.;
        app.camera.pitch = std::f64::consts::PI / 6.;
        app.camera.yaw = -std::f64::consts::FRAC_PI_2;
        app.camera.perspective = true;
        if mode == "group" {
            let mesh=std::mem::take(&mut app.scene.mesh);
            app.scene.groups.push(ingetrazo_core::groups::Group::with_mesh("moving curve group",mesh));
            if std::env::var_os("INGETRAZO_GROUP_LINKED").is_some() {
                app.scene.make_component(app.scene.groups[0].id).unwrap();
                let mut sibling=app.scene.groups[0].duplicate_instance();
                sibling.transform=glam::DMat4::from_translation(DVec3::X*10.);
                app.scene.groups.push(sibling);
            }
            if std::env::var_os("INGETRAZO_GROUP_NESTED").is_some() {
                let child=app.scene.groups.remove(0);
                let mut outer=ingetrazo_core::groups::Group::new("linked outer context");outer.children.push(child);if std::env::var_os("INGETRAZO_CONTEXT_EXIT").is_some(){outer.transform=glam::DMat4::from_translation(DVec3::new(1.,-1.,0.));}
                app.scene.groups.insert(0,outer);app.scene.make_component(app.scene.groups[0].id).unwrap();
                let mut sibling=app.scene.groups[0].duplicate_instance();sibling.transform=glam::DMat4::from_translation(DVec3::X*10.)*sibling.transform;app.scene.groups.push(sibling);
                app.scene.select(ingetrazo_core::scene::SelectionItem::Group(0));app.begin_component_edit();
            }
            app.scene.select(ingetrazo_core::scene::SelectionItem::Group(0));
        }
        app.set_tool(if mode=="group"{ToolType::Move}else{ToolType::Line});
        app.mark_document_clean();
        Self {
            app,
            dir: path.unwrap(),
            mode,
        }
    }
    fn build_ui(&mut self, ctx: &egui::Context) {
        self.app.update_ui(ctx);
        if let Some(rect) = self.app.native_gpu_viewport_rect {
            let matrix = self.app.camera.projection_matrix() * self.app.camera.view_matrix();
            let project = |p: DVec3| {
                let c = matrix * p.extend(1.);
                let n = c.truncate() / c.w;
                [
                    rect.left() as f64 + (n.x + 1.) * 0.5 * rect.width() as f64,
                    rect.top() as f64 + (1. - n.y) * 0.5 * rect.height() as f64,
                ]
            };
            let group_centers=self.app.scene.groups.iter().map(|g|{
                let local=g.mesh.vertices.iter().map(|v|v.position).sum::<DVec3>()/g.mesh.vertices.len().max(1) as f64;
                g.transform.transform_point3(local).to_array()
            }).collect::<Vec<_>>();
            let center = if self.mode=="group" {group_centers.first().map(|p|DVec3::from_array(*p)).unwrap_or(DVec3::new(2.,3.,1.))} else if self.app.scene.mesh.vertices.len() >= 24 {
                self.app.scene.mesh.vertices[..24]
                    .iter()
                    .map(|v| v.position)
                    .sum::<DVec3>()
                    / 24.
            } else {
                DVec3::new(2., 3., 1.)
            };
            let copy_ids=self.app.scene.mesh.edges.iter().filter(|e|e.curve_id.is_some_and(|id|id!=71)).flat_map(|e|[e.v0,e.v1]).collect::<std::collections::HashSet<_>>();
            let copy_center=(!copy_ids.is_empty()).then(||copy_ids.iter().map(|&id|self.app.scene.mesh.vertices[id].position).sum::<DVec3>()/copy_ids.len() as f64);
            let radius = if self.mode == "tiny" { 0.01 } else { 1.2 };
            let rim = center + DVec3::X * radius;
            let copy_probe=if self.app.active_tool.rubber_band_lines().len()>1{self.app.active_tool.snap_anchor().zip(self.app.snap_engine.current_inference()).map(|(grab,r)|project(center+r.point-grab+DVec3::Y*radius))}else{None};
            fn projected_wires(group:&ingetrazo_core::groups::Group,parent:glam::DMat4,project:&impl Fn(DVec3)->[f64;2],out:&mut Vec<serde_json::Value>){
                let world=parent*group.transform;
                out.push(serde_json::json!({"group_id":group.id.to_string(),"segments":group.mesh.edges.iter().filter(|e|!e.hidden).map(|e|[project(world.transform_point3(group.mesh.vertices[e.v0].position)),project(world.transform_point3(group.mesh.vertices[e.v1].position))]).collect::<Vec<_>>()}));
                for child in &group.children{projected_wires(child,world,project,out);}
            }
            let mut committed_wire_segments=Vec::new();
            for group in &self.app.scene.groups{projected_wires(group,glam::DMat4::IDENTITY,&project,&mut committed_wire_segments);}
            let preview_wire_segments=self.app.active_tool.rubber_band_lines().iter().map(|(a,b)|[project(*a),project(*b)]).collect::<Vec<_>>();
            let state = serde_json::json!({"viewport_rect":[rect.left(),rect.top(),rect.width(),rect.height()],"committed_wire_segments":committed_wire_segments,"preview_wire_segments":preview_wire_segments,"active_world_centers":group_centers.iter().map(|p|self.app.scene.dimension_frame.transform_point3(DVec3::from_array(*p)).to_array()).collect::<Vec<_>>(),"root_child_counts":self.app.scene.groups.iter().map(|g|g.children.len()).collect::<Vec<_>>(),"root_child_world_centers":self.app.scene.groups.iter().map(|g|g.children.iter().map(|c|{let local=c.mesh.vertices.iter().map(|v|v.position).sum::<DVec3>()/c.mesh.vertices.len().max(1) as f64;(g.transform*c.transform).transform_point3(local).to_array()}).collect::<Vec<_>>()).collect::<Vec<_>>(),"context_depth":self.app.component_edit_stack.len(),"definitions":self.app.scene.component_definitions,"group_definitions":self.app.scene.groups.iter().map(|g|g.component_definition).collect::<Vec<_>>(),"group_centers":group_centers,"group_ids":self.app.scene.groups.iter().map(|g|g.id.to_string()).collect::<Vec<_>>(),"group_count":self.app.scene.groups.len(),"mode":self.mode,"vertices":self.app.scene.mesh.vertices.len(),"unused_vertices":self.app.scene.mesh.vertices.iter().filter(|v|v.edges.is_empty()).count(),"copy_probe_pixel":copy_probe,"copy_center":copy_center.map(|p|p.to_array()),"curves":self.app.scene.mesh.edges.iter().map(|e|e.curve_id).collect::<Vec<_>>(),"preview_lines":self.app.active_tool.rubber_band_lines().len(),"time":ctx.input(|i|i.time),"center":center.to_array(),"rim":project(rim),"center_pixel":project(center),"move_target":project(rim+DVec3::X*std::env::var("INGETRAZO_MOVE_DISTANCE").ok().and_then(|v|v.parse::<f64>().ok()).unwrap_or(1.)),"kind":self.app.snap_engine.current_inference().map(|s|format!("{:?}",s.kind)),"inference_point":self.app.snap_engine.current_inference().map(|s|s.point.to_array()),"memory":self.app.snap_engine.encouraged_points.iter().map(|p|p.to_array()).collect::<Vec<_>>(),"edges":self.app.scene.mesh.edges.len()+self.app.scene.groups.iter().map(|g|g.mesh.edges.len()).sum::<usize>(),"faces":self.app.scene.mesh.faces.len(),"undo":self.app.undo_stack.len(),"anchor":self.app.active_tool.snap_anchor().map(|p|p.to_array()),"tool":format!("{:?}",self.app.active_tool_type),"revision":self.app.model_revision});
            std::fs::write(
                self.dir.join("state.tmp"),
                serde_json::to_vec(&state).unwrap(),
            )
            .unwrap();
            std::fs::rename(self.dir.join("state.tmp"),self.dir.join("state.json")).unwrap();
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
    fn prepare_gpu(&mut self, system: &mut System, resize: bool) {
        App::prepare_gpu(&mut self.app, system, resize)
    }
    fn gpu_error(&mut self, error: &str) {
        panic!("Native fixture GPU error: {error}");
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).unwrap();
    ingetrazo_gui::native_vulkan::run_app::<Fixture>(Some(dir.into()))
}
