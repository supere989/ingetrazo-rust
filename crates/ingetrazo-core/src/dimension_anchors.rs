//! Runtime vertex holds matching original dimension binding and fallback.
use glam::{DMat4,DVec3};use uuid::Uuid;use crate::{scene::Scene,groups::Group,mesh::Mesh,dimensions::Dimension};
#[derive(Debug,Clone,PartialEq)]
pub struct VertexAnchor {pub vertex:Uuid,pub path:Vec<Uuid>}
fn at(mesh:&Mesh,m:DMat4,p:DVec3,path:&[Uuid])->Option<VertexAnchor>{mesh.vertices.iter().find(|v|!v.edges.is_empty()&&(m.transform_point3(v.position)-p).length()<1e-6).map(|v|VertexAnchor{vertex:v.uid,path:path.to_vec()})}
fn find(groups:&[Group],parent:DMat4,p:DVec3,path:&mut Vec<Uuid>)->Option<VertexAnchor>{for group in groups{let m=parent*group.transform;path.push(group.id);let anchor=at(&group.mesh,m,p,path).or_else(||find(&group.children,m,p,path));path.pop();if anchor.is_some(){return anchor;}}None}
impl VertexAnchor {
    pub fn position(&self,scene:&Scene)->Option<DVec3>{
        let mut mesh=&scene.mesh;let mut groups=&scene.groups;let mut matrix=DMat4::IDENTITY;
        for id in &self.path{let group=groups.iter().find(|g|g.id==*id)?;matrix*=group.transform;mesh=&group.mesh;groups=&group.children;}
        let vertex=mesh.vertices.iter().find(|v|v.uid==self.vertex&&!v.edges.is_empty())?;Some(matrix.transform_point3(vertex.position))
    }
}
impl Scene {
    pub fn resolve_dimension_anchor(&self,point:DVec3)->Option<VertexAnchor>{at(&self.mesh,DMat4::IDENTITY,point,&[]).or_else(||find(&self.groups,DMat4::IDENTITY,point,&mut vec![]))}
    pub fn bind_dimensions(&mut self){let mut dimensions=std::mem::take(&mut self.dimensions);for dim in &mut dimensions{dim.anchor_a=self.resolve_dimension_anchor(dim.start);dim.anchor_b=self.resolve_dimension_anchor(dim.end);}self.dimensions=dimensions;}
    pub fn dimension_points(&self,dim:&Dimension)->(DVec3,DVec3){(dim.anchor_a.as_ref().and_then(|a|a.position(self)).unwrap_or(dim.start),dim.anchor_b.as_ref().and_then(|a|a.position(self)).unwrap_or(dim.end))}
    pub fn refresh_dimensions(&mut self){
        let mut dimensions=std::mem::take(&mut self.dimensions);
        for dim in &mut dimensions{for(point,anchor)in[(&mut dim.start,&mut dim.anchor_a),(&mut dim.end,&mut dim.anchor_b)]{
            if let Some(held)=anchor.as_ref(){if let Some(live)=held.position(self){*point=live;}else{*anchor=self.resolve_dimension_anchor(*point);if let Some(live)=anchor.as_ref().and_then(|a|a.position(self)){*point=live;}}}
        }}self.dimensions=dimensions;
    }
}
#[cfg(test)]mod tests{use super::*;use crate::scene::SelectionItem;
    #[test]fn anchors_follow_uid_through_movement_grouping_transform_explode_and_deletion(){
        let mut scene=Scene::new();let a=scene.mesh.add_vertex(DVec3::ZERO);let b=scene.mesh.add_vertex(DVec3::X);scene.mesh.add_edge(a,b);
        scene.dimensions.push(Dimension::new(DVec3::ZERO,DVec3::X,DVec3::Y));scene.bind_dimensions();assert!(scene.dimensions[0].anchor_a.is_some());
        scene.mesh.vertices[b].position.x=3.;scene.refresh_dimensions();assert_eq!(scene.dimensions[0].end.x,3.);
        scene.select(SelectionItem::Edge(0));scene.group_selection("part").unwrap();scene.refresh_dimensions();assert_eq!(scene.dimensions[0].anchor_b.as_ref().unwrap().path,vec![scene.groups[0].id]);
        scene.groups[0].transform=DMat4::from_scale(DVec3::splat(2.));scene.refresh_dimensions();assert_eq!(scene.dimensions[0].end.x,6.);
        scene.explode_selected_groups().unwrap();scene.refresh_dimensions();assert!(scene.dimensions[0].anchor_b.as_ref().unwrap().path.is_empty());
        scene.mesh=Mesh::new();scene.refresh_dimensions();assert!(scene.dimensions[0].anchor_b.is_none());assert_eq!(scene.dimensions[0].end.x,6.);
        scene.mesh.add_vertex(DVec3::new(100.,0.,0.));scene.refresh_dimensions();assert_eq!(scene.dimensions[0].end.x,6.);
    }
}
