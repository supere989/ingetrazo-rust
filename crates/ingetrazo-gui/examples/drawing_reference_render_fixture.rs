// SPDX-License-Identifier: GPL-3.0-or-later
use glam::DVec3;
use ingetrazo_core::camera::StandardView;
use ingetrazo_gui::{viewport::render_viewport, IngeTrazoApp};
use ingetrazo_tools::base::ToolType;
struct Harness {
    app: IngeTrazoApp,
    dir: std::path::PathBuf,
    capture: String,
}
impl eframe::App for Harness {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx,|ui|{let rect=ui.available_rect_before_wrap();render_viewport(&mut self.app,ui);
 let matrix=self.app.camera.projection_matrix()*self.app.camera.view_matrix();let project=|p:DVec3|{let c=matrix*p.extend(1.);let n=c.truncate()/c.w;[rect.left() as f64+(n.x+1.)*0.5*rect.width()as f64,rect.top()as f64+(1.-n.y)*0.5*rect.height()as f64]};
 let origin=DVec3::new(2.5,3.,1.5);let normal=DVec3::new(1.,2.,3.).normalize();let u=(DVec3::X-normal*normal.x).normalize();let v=normal.cross(u).normalize();
 let rows=serde_json::json!({"faces":self.app.scene.mesh.faces.len(),"edges":self.app.scene.mesh.edges.len(),"undo":self.app.undo_stack.len(),"pending_anchor":self.app.active_tool.snap_anchor().map(|p|p.to_array()),"drawing_plane":self.app.active_tool.drawing_plane().map(|(p,n)|[p.to_array(),n.to_array()]),"preview":self.app.active_tool.rubber_band_lines().iter().map(|(a,b)|[a.to_array(),b.to_array()]).collect::<Vec<_>>(),"click":project(origin),"move":project(origin+u*1.5+v*1.2),"origin":origin.to_array(),"normal":normal.to_array(),"face_areas":(0..self.app.scene.mesh.faces.len()).map(|id|self.app.scene.mesh.face_area(id)).collect::<Vec<_>>(),"face_normals":(0..self.app.scene.mesh.faces.len()).map(|id|self.app.scene.mesh.face_normal(id).to_array()).collect::<Vec<_>>(),"vertices":self.app.scene.mesh.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"pointer":ctx.input(|i|i.pointer.hover_pos().map(|p|[p.x,p.y]))});
 if self.app.scene.mesh.faces.len()==1&&!self.dir.join("committed.igz").exists(){ingetrazo_formats::igz::save_igz(&self.app.scene,&self.dir.join("committed.igz")).unwrap();}
 std::fs::write(self.dir.join("state.tmp"),serde_json::to_vec(&rows).unwrap()).unwrap();
 std::fs::rename(self.dir.join("state.tmp"),self.dir.join("state.json")).unwrap();
 });
        if let Ok(name) = std::fs::read_to_string(self.dir.join("capture.txt")) {
            if name != self.capture {
                self.capture = name;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(())));
            }
        }
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event {
                let pixels: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                image::save_buffer(
                    self.dir.join(format!("{}.png", self.capture)),
                    &pixels,
                    image.size[0] as u32,
                    image.size[1] as u32,
                    image::ColorType::Rgba8,
                )
                .unwrap();
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(30));
    }
}
fn main() -> eframe::Result {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let mut app = IngeTrazoApp::new();
    let a = app.scene.mesh.add_vertex(DVec3::new(2., 2., 0.));
    let b = app.scene.mesh.add_vertex(DVec3::new(3., 4., 3.));
    app.scene.mesh.add_edge(a, b);
    app.camera.set_standard_view(StandardView::Top);
    app.camera.perspective = false;
    app.camera.target = DVec3::new(3., 3., 0.);
    app.camera.distance = 8.;
    app.set_tool(ToolType::Rectangle);
    eframe::run_native(
        "Drawing reference viewport",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1000., 700.]),
            ..Default::default()
        },
        Box::new(move |_| {
            Ok(Box::new(Harness {
                app,
                dir,
                capture: String::new(),
            }))
        }),
    )
}
