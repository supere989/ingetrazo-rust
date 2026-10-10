//! Visible world-space geometry shared by mesh exchange writers.
use glam::{DMat4,DVec3};
use ingetrazo_core::{scene::Scene,mesh::Mesh,groups::Group};

pub(crate) struct Triangle { pub points:[DVec3;3], pub local_points:[DVec3;3], pub attrs:std::collections::HashMap<String,serde_json::Value>, pub layer:String }
pub(crate) fn triangles(scene:&Scene)->Vec<Triangle> {
    fn visible(scene:&Scene,layer:Option<&str>)->bool {
        layer.and_then(|name|scene.layers.iter().find(|l|l.name==name)).map_or(true,|l|l.visible)
    }
    fn mesh(scene:&Scene,mesh:&Mesh,world:DMat4,inherited:Option<&str>,out:&mut Vec<Triangle>) {
        for (id,face) in mesh.faces.iter().enumerate() {
            let layer=face.layer.as_deref().or(inherited);
            if !visible(scene,layer) || face.attrs.get("hidden").and_then(|v|v.as_bool())==Some(true) {continue;}
            for (a,b,c) in mesh.triangulate_face(id) {
                let mut local_points=[a,b,c];
                let mut points=local_points.map(|p|world.transform_point3(p));
                if world.determinant()<0. {points.swap(1,2);local_points.swap(1,2);}
                out.push(Triangle{points,local_points,attrs:face.attrs.clone(),layer:layer.unwrap_or("0").into()});
            }
        }
    }
    fn group(scene:&Scene,g:&Group,parent:DMat4,inherited:Option<&str>,out:&mut Vec<Triangle>) {
        let layer=g.layer.as_deref().or(inherited);
        if g.hidden || g.billboard || !visible(scene,layer) {return;}
        let world=parent*g.transform;
        mesh(scene,&g.mesh,world,layer,out);
        for child in &g.children {group(scene,child,world,layer,out);}
    }
    let mut out=Vec::new();mesh(scene,&scene.mesh,DMat4::IDENTITY,None,&mut out);
    for g in &scene.groups {group(scene,g,DMat4::IDENTITY,None,&mut out);}
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::layers::Layer;
    #[test]
    fn nested_holes_reflections_and_hidden_containers_preserve_world_surface() {
        let mut mesh=Mesh::new();
        let outer=[[0.,0.],[4.,0.],[4.,4.],[0.,4.]].map(|[x,y]|mesh.add_vertex(DVec3::new(x,y,0.)));
        let hole=[[1.,1.],[1.,3.],[3.,3.],[3.,1.]].map(|[x,y]|mesh.add_vertex(DVec3::new(x,y,0.)));
        mesh.add_face(outer.to_vec(),vec![hole.to_vec()]).unwrap();
        let mut child=Group::with_mesh("hole",mesh);
        child.transform=DMat4::from_scale(DVec3::new(-2.,3.,1.));
        let mut parent=Group::new("parent");parent.transform=DMat4::from_translation(DVec3::new(10.,20.,5.));parent.children.push(child);
        let mut scene=Scene::new();scene.groups.push(parent);
        let tris=triangles(&scene);
        let area:f64=tris.iter().map(|t|{let [a,b,c]=t.points;let n=(b-a).cross(c-a);assert!(n.z>0.);n.length()/2.}).sum();
        assert!((area-72.).abs()<1e-9);
        for t in &tris {for p in t.points {assert!(p.x>=2. && p.x<=10. && p.y>=20. && p.y<=32.);assert_eq!(p.z,5.);}}
        scene.groups[0].hidden=true;assert!(triangles(&scene).is_empty());
        scene.groups[0].hidden=false;scene.groups[0].layer=Some("off".into());let mut off=Layer::new("off");off.visible=false;scene.layers.push(off);
        assert!(triangles(&scene).is_empty());
    }
}
