//! Command-specific AddEdge ownership; Undo removes its edge, not welded vertices.
use crate::IngeTrazoApp;
use glam::DVec3;
impl IngeTrazoApp {
    pub fn add_edge_command(&mut self, a:DVec3, b:DVec3, soft:Option<bool>, curve:Option<usize>) -> Result<(),String> {
        let a=a.as_vec3().as_dvec3(); let b=b.as_vec3().as_dvec3();
        if !a.is_finite() || !b.is_finite() {return Err("Edge endpoints must be finite Qt coordinates".into());}
        if ingetrazo_core::math::PointKey::from_vec3(a)==ingetrazo_core::math::PointKey::from_vec3(b) {
            return Err("Edge endpoints weld to the same vertex".into());
        }
        // Admission is complete before any publication. Native add_vertex/add_edge
        // cannot refuse distinct validated keys. Only the existing history Scene
        // snapshot is copied, not additional whole-mesh candidates per endpoint.
        self.save_undo_state();
        self.mark_visibility_delta();
        let old_edges=self.scene.mesh.edges.len();
        let v0=self.scene.mesh.add_vertex(a);let v1=self.scene.mesh.add_vertex(b);
        let before=&mut self.undo_stack.last_mut().expect("saved command snapshot").mesh;
        for vertex in [v0,v1] {
            let actual=&self.scene.mesh.vertices[vertex];
            let id=before.add_vertex(actual.position);
            before.vertices[id].uid=actual.uid;
        }
        let id=self.scene.mesh.add_edge(v0,v1).expect("distinct admitted vertex keys");
        if let Some(soft)=soft {self.scene.mesh.edges[id].soft=soft;}
        if let Some(curve)=curve {self.scene.mesh.edges[id].curve_id=Some(curve);}
        // A reused edge is not command-owned, including its optional flag stamp.
        if id<old_edges {before.edges[id]=self.scene.mesh.edges[id].clone();}
        self.scene.bump_version();
        Ok(())
    }
}
#[cfg(test)] mod tests {
 use super::*;
 #[test] fn edge_undo_keeps_vertex_identity_and_merged_flags() {
  let mut app=IngeTrazoApp::new();app.add_edge_command(DVec3::ZERO,DVec3::X,Some(true),Some(7)).unwrap();
  let ids:Vec<_>=app.scene.mesh.vertices.iter().map(|v|v.uid).collect();
  app.undo();assert!(app.scene.mesh.edges.is_empty());assert_eq!(app.scene.mesh.vertices.iter().map(|v|v.uid).collect::<Vec<_>>(),ids);
  app.redo();assert_eq!(app.scene.mesh.edges.len(),1);assert!(app.scene.mesh.edges[0].soft);
  app.add_edge_command(DVec3::X,DVec3::ZERO,Some(false),Some(9)).unwrap();app.undo();assert_eq!(app.scene.mesh.edges.len(),1);assert!(!app.scene.mesh.edges[0].soft);assert_eq!(app.scene.mesh.edges[0].curve_id,Some(9));
 }
 #[test] fn invalid_edge_has_no_document_or_history_mutation() {
  let mut app=IngeTrazoApp::new();let before=app.scene.clone();assert!(app.add_edge_command(DVec3::ZERO,DVec3::ZERO,None,None).is_err());assert_eq!(app.scene,before);assert!(app.undo_stack.is_empty());
 }
}
