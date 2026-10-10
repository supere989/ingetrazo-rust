//! Command-specific AddEdge ownership; Undo removes its edge, not welded vertices.
use crate::IngeTrazoApp;
use glam::DVec3;
impl IngeTrazoApp {
    pub fn add_edge_command(&mut self, a:DVec3, b:DVec3, soft:Option<bool>, curve:Option<usize>) -> Result<(),String> {
        let a=a.as_vec3().as_dvec3(); let b=b.as_vec3().as_dvec3();
        if !a.is_finite() || !b.is_finite() {return Err("Edge endpoints must be finite Qt coordinates".into());}
        let mut candidate=self.scene.mesh.clone();
        let v0=candidate.add_vertex(a);let v1=candidate.add_vertex(b);
        if v0==v1 {return Err("Edge endpoints weld to the same vertex".into());}
        let existing=candidate.find_edge(v0,v1);
        // The command owns neither welded endpoint. Preserve those exact UIDs
        // through Undo, including endpoints newly interned by this command.
        let mut undo_mesh=candidate.clone();
        let id=candidate.add_edge(v0,v1).ok_or("Invalid edge endpoints")?;
        if let Some(soft)=soft {candidate.edges[id].soft=soft;}
        if let Some(curve)=curve {candidate.edges[id].curve_id=Some(curve);}
        if existing.is_some() {undo_mesh=candidate.clone();}
        self.save_undo_state();
        self.undo_stack.last_mut().expect("saved command snapshot").mesh=undo_mesh;
        self.scene.mesh=candidate;self.scene.bump_version();
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
