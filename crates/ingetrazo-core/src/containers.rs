//! Attribute-preserving container extraction and one-level explosion.
use std::collections::{HashMap,HashSet};
use glam::DMat4;
use crate::{groups::Group,mesh::Mesh,scene::{Scene,SelectionItem}};

fn pair(a:usize,b:usize)->(usize,usize) {if a<b {(a,b)} else {(b,a)}}
fn boundaries(mesh:&Mesh,faces:&HashSet<usize>)->HashSet<(usize,usize)> {
    let mut pairs=HashSet::new();
    for &fid in faces {
        if let Some(f)=mesh.faces.get(fid) {
            for ring in std::iter::once(&f.vertices).chain(&f.holes) {
                for i in 0..ring.len() {pairs.insert(pair(ring[i],ring[(i+1)%ring.len()]));}
            }
        }
    }
    pairs
}

/// Copy explicit subsets and rebuild every vertex/edge/face incidence. Opaque
/// edge/face data follow entities rather than array positions.
fn append_subset(dst:&mut Mesh,src:&Mesh,faces:&HashSet<usize>,edges:&HashSet<usize>,transform:DMat4,
                 layer:Option<&str>,material:Option<&serde_json::Value>,hidden:bool)->Result<(),String> {
    if !transform.is_finite() || transform.determinant().abs()<1e-12 {return Err("invalid container transform".into());}
    let mut map=HashMap::new();
    fn vertex(dst:&mut Mesh,src:&Mesh,map:&mut HashMap<usize,usize>,id:usize,t:DMat4)->Result<usize,String> {
        if let Some(&mapped)=map.get(&id) {return Ok(mapped);}
        let p=src.vertices.get(id).ok_or("invalid vertex reference")?.position;
        let p=t.transform_point3(p);if !p.is_finite() {return Err("non-finite container geometry".into());}
        let mapped=dst.add_vertex(p);map.insert(id,mapped);Ok(mapped)
    }
    // Preserve deterministic original ordering, despite unordered selection.
    for fid in 0..src.faces.len() {
        if !faces.contains(&fid) {continue;}
        let f=&src.faces[fid];
        let verts=f.vertices.iter().map(|&v|vertex(dst,src,&mut map,v,transform)).collect::<Result<Vec<_>,_>>()?;
        let holes=f.holes.iter().map(|ring|ring.iter().map(|&v|vertex(dst,src,&mut map,v,transform)).collect::<Result<Vec<_>,_>>()).collect::<Result<Vec<_>,_>>()?;
        let id=dst.add_face(verts,holes).ok_or("invalid container face")?;
        dst.faces[id].attrs=f.attrs.clone();
        dst.faces[id].layer=f.layer.clone().or_else(||layer.map(str::to_owned));
        if let Some(mat)=material {dst.faces[id].attrs.entry("material".into()).or_insert_with(||mat.clone());}
        if hidden {dst.faces[id].attrs.insert("hidden".into(),serde_json::json!(true));}
    }
    for eid in 0..src.edges.len() {
        if !edges.contains(&eid) {continue;}
        let e=&src.edges[eid];
        let a=vertex(dst,src,&mut map,e.v0,transform)?;let b=vertex(dst,src,&mut map,e.v1,transform)?;
        let id=dst.add_edge(a,b).ok_or("invalid container edge")?;
        let target=&mut dst.edges[id];
        target.attrs=e.attrs.clone();target.soft=e.soft;target.hidden=e.hidden||hidden;
        target.curve_id=e.curve_id;target.layer=e.layer.clone().or_else(||layer.map(str::to_owned));
    }
    Ok(())
}

/// A complete affine copy retains opaque entity data and positioned texture
/// covectors. Used when clipboard geometry crosses document editing frames.
pub fn transform_mesh(source:&Mesh,transform:DMat4)->Result<Mesh,String>{
    if !transform.is_finite()||transform.determinant().abs()<1e-12{return Err("invalid clipboard coordinate frame".into());}
    let mut copy=source.clone();for vertex in &mut copy.vertices{vertex.position=transform.transform_point3(vertex.position);if !vertex.position.is_finite(){return Err("non-finite clipboard geometry".into());}}
    let inverse_transpose=transform.inverse().transpose();let translation=transform.w_axis.truncate();
    for face in &mut copy.faces{if let Some(texture)=face.attrs.get_mut("texture"){if let Some(values)=texture.get("uvw").and_then(|v|v.as_array()).filter(|a|a.len()==8).and_then(|a|a.iter().map(|v|v.as_f64()).collect::<Option<Vec<_>>>()){
        let mut mapped=values.clone();for base in[0,4]{let gradient=inverse_transpose.transform_vector3(glam::DVec3::new(values[base],values[base+1],values[base+2]));mapped[base]=gradient.x;mapped[base+1]=gradient.y;mapped[base+2]=gradient.z;mapped[base+3]=values[base+3]-gradient.dot(translation);}texture["uvw"]=serde_json::json!(mapped);
    }}}
    copy.rebuild_spatial_index();Ok(copy)
}

/// Source shareback reconstructs referenced entities and partitions curves.
/// Clipboard snapshots deliberately retain a separate copy policy.
pub fn source_shareback_mesh(source: &Mesh, transform: DMat4) -> Result<Mesh, String> {
    // Native-authored f64 placement frames cannot silently become Qt f32.
    let qt_precision = transform == transform.as_mat4().as_dmat4();
    let mut mesh = crate::change_axes::transformed_policy(source, transform, qt_precision, true)?;
    let mut by_id = std::collections::BTreeMap::<usize, Vec<usize>>::new();
    for (id, e) in mesh.edges.iter().enumerate() {
        if let Some(c) = e.curve_id {
            by_id.entry(c).or_default().push(id);
        }
    }
    let mut next = mesh
        .edges
        .iter()
        .filter_map(|e| e.curve_id)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or("Curve identity exhausted")?;
    for (curve, edges) in by_id {
        let set: HashSet<_> = edges.iter().copied().collect();
        let is_break = |v: usize| {
            let incident = &mesh.vertices[v].edges;
            let degree = incident.iter().filter(|id| set.contains(id)).count();
            degree != 2 || incident.len() != degree
        };
        let seeds = edges
            .iter()
            .copied()
            .filter(|&id| {
                let e = &mesh.edges[id];
                is_break(e.v0) || is_break(e.v1)
            })
            .chain(edges.iter().copied())
            .collect::<Vec<_>>();
        let mut unvisited = set.clone();
        let mut chains = Vec::new();
        for seed in seeds {
            if !unvisited.contains(&seed) {
                continue;
            }
            let edge = &mesh.edges[seed];
            let mut v = if is_break(edge.v0) {
                edge.v0
            } else if is_break(edge.v1) {
                edge.v1
            } else {
                edge.v0
            };
            let mut id = seed;
            let mut chain = Vec::new();
            while unvisited.remove(&id) {
                chain.push(id);
                let e = &mesh.edges[id];
                v = if e.v0 == v { e.v1 } else { e.v0 };
                if is_break(v) {
                    break;
                }
                let Some(other) = mesh.vertices[v]
                    .edges
                    .iter()
                    .copied()
                    .find(|id| unvisited.contains(id))
                else {
                    break;
                };
                id = other;
            }
            chains.push(chain);
        }
        if chains.len() <= 1 {
            continue;
        }
        for (i, chain) in chains.into_iter().enumerate() {
            let id = if i == 0 {
                curve
            } else {
                let id = next;
                next = next.checked_add(1).ok_or("Curve identity exhausted")?;
                id
            };
            for edge in chain {
                mesh.edges[edge].curve_id = Some(id);
            }
        }
    }
    Ok(mesh)
}

/// Extract explicit loose faces/edges, including all copied face boundaries.
/// Transform tools use this without cloning unrelated Scene data at grab time.
pub fn copy_mesh_subset(source:&Mesh,faces:&HashSet<usize>,selected_edges:&HashSet<usize>)->Result<Mesh,String>{
    if faces.iter().any(|i|*i>=source.faces.len())||selected_edges.iter().any(|i|*i>=source.edges.len()){return Err("invalid geometry selection".into());}
    let mut edges=selected_edges.clone();let pairs=boundaries(source,faces);
    for (i,e) in source.edges.iter().enumerate(){if pairs.contains(&pair(e.v0,e.v1)){edges.insert(i);}}
    let mut mesh=Mesh::new();append_subset(&mut mesh,source,faces,&edges,DMat4::IDENTITY,None,None,false)?;Ok(mesh)
}
/// Copy selected loose geometry, including every selected face boundary.
pub fn copy_selected_mesh(scene:&Scene)->Result<Mesh,String> {
    let faces:HashSet<_>=scene.selection.iter().filter_map(|s|if let SelectionItem::Face(i)=s {Some(*i)}else{None}).collect();
    let edges:HashSet<_>=scene.selection.iter().filter_map(|s|if let SelectionItem::Edge(i)=s {Some(*i)}else{None}).collect();
    copy_mesh_subset(&scene.mesh,&faces,&edges)
}
/// Append a translated snapshot atomically; remap curve identities independently
/// and keep attributes on pre-existing destination edges untouched.
pub fn paste_mesh(destination:&Mesh,source:&Mesh,offset:glam::DVec3)->Result<(Mesh,Vec<usize>,Vec<usize>),String> {
    if !offset.is_finite(){return Err("invalid clipboard offset".into());}
    let mut mesh=destination.clone();let mut copy=source.clone();
    let mut next=destination.edges.iter().filter_map(|e|e.curve_id.or_else(||e.attrs.get("curve").and_then(|v|v.as_u64()).map(|i|i as usize))).max().unwrap_or(0).checked_add(1).ok_or("curve identity exhausted")?;
    let mut curves=HashMap::new();
    for edge in &mut copy.edges {
        if let Some(old)=edge.curve_id.or_else(||edge.attrs.get("curve").and_then(|v|v.as_u64()).map(|i|i as usize)) {
            let fresh=if let Some(id)=curves.get(&old){*id}else{let id=next;next=next.checked_add(1).ok_or("curve identity exhausted")?;curves.insert(old,id);id};
            edge.curve_id=Some(fresh);edge.attrs.insert("curve".into(),serde_json::json!(fresh));
        }
    }
    for face in &mut copy.faces {
        if let Some(texture)=face.attrs.get_mut("texture") {
            if let Some(values)=texture.get("uvw").and_then(|v|v.as_array()).filter(|v|v.len()==8).and_then(|v|v.iter().map(|x|x.as_f64()).collect::<Option<Vec<_>>>()) {
                let mut mapped=values.clone();for base in [0,4]{mapped[base+3]-=glam::DVec3::new(values[base],values[base+1],values[base+2]).dot(offset);}
                texture["uvw"]=serde_json::json!(mapped);
            }
        }
    }
    let old_faces=mesh.faces.len();let old_edges=mesh.edges.len();
    append_subset(&mut mesh,&copy,&(0..copy.faces.len()).collect(),&(0..copy.edges.len()).collect(),DMat4::from_translation(offset),None,None,false)?;
    // add_edge welds by endpoints; copying must never repaint originals.
    for (target,original) in mesh.edges.iter_mut().take(old_edges).zip(&destination.edges){let faces=target.faces.clone();*target=original.clone();target.faces=faces;}
    let faces=(old_faces..mesh.faces.len()).collect();let edges=(old_edges..mesh.edges.len()).collect();mesh.rebuild_spatial_index();Ok((mesh,faces,edges))
}

impl Scene {
    /// Build the prospective meshes first, then move ownership atomically.
    pub fn group_selection(&mut self,name:impl Into<String>)->Result<usize,String> {
        let mut selected_faces=HashSet::new();let mut selected_edges=HashSet::new();let mut selected_groups=HashSet::new();
        for item in &self.selection {
            match *item {
                SelectionItem::Face(id) if id<self.mesh.faces.len()=>{selected_faces.insert(id);},
                SelectionItem::Edge(id) if id<self.mesh.edges.len()=>{selected_edges.insert(id);},
                SelectionItem::Group(id) if id<self.groups.len()=>{selected_groups.insert(id);},
                _=>{}
            }
        }
        if selected_faces.is_empty() && selected_edges.is_empty() && selected_groups.is_empty() {return Err("Select faces, edges or groups to make a group".into());}
        let moved_boundaries=boundaries(&self.mesh,&selected_faces);
        for (i,e) in self.mesh.edges.iter().enumerate() {
            if moved_boundaries.contains(&pair(e.v0,e.v1)) {selected_edges.insert(i);}
        }
        let kept_faces:HashSet<_>=(0..self.mesh.faces.len()).filter(|i|!selected_faces.contains(i)).collect();
        let kept_boundaries=boundaries(&self.mesh,&kept_faces);
        let kept_edges:HashSet<_>=self.mesh.edges.iter().enumerate().filter_map(|(i,e)|
            if !selected_edges.contains(&i)||kept_boundaries.contains(&pair(e.v0,e.v1)) {Some(i)} else {None}).collect();
        let mut group_mesh=Mesh::new();let mut root_mesh=Mesh::new();
        append_subset(&mut group_mesh,&self.mesh,&selected_faces,&selected_edges,DMat4::IDENTITY,None,None,false)?;
        append_subset(&mut root_mesh,&self.mesh,&kept_faces,&kept_edges,DMat4::IDENTITY,None,None,false)?;
        let mut group=Group::with_mesh(name,group_mesh);
        let mut remaining=Vec::new();
        for (i,child) in std::mem::take(&mut self.groups).into_iter().enumerate() {
            if selected_groups.contains(&i) {group.children.push(child);} else {remaining.push(child);}
        }
        let index=remaining.len();remaining.push(group);self.groups=remaining;self.mesh=root_mesh;
        self.clear_selection();self.select(SelectionItem::Group(index));self.bump_version();Ok(index)
    }

    /// Explode exactly one nesting level. Child placements are promoted while
    /// inherited transforms/paint/layers remain effective; shared definitions
    /// themselves are never edited by exploding an instance.
    pub fn explode_selected_groups(&mut self)->Result<(),String> {
        let selected:HashSet<_>=self.selection.iter().filter_map(|s|if let SelectionItem::Group(i)=s {
            if *i<self.groups.len() {Some(*i)} else {None}
        } else {None}).collect();
        if selected.is_empty() {return Err("Select a group to explode".into());}
        let mut prospective=Mesh::new();
        let all_faces=(0..self.mesh.faces.len()).collect();let all_edges=(0..self.mesh.edges.len()).collect();
        append_subset(&mut prospective,&self.mesh,&all_faces,&all_edges,DMat4::IDENTITY,None,None,false)?;
        let mut groups=Vec::new();
        for (i,g) in self.groups.iter().enumerate() {
            if !selected.contains(&i) {groups.push(g.clone());continue;}
            let faces=(0..g.mesh.faces.len()).collect();let edges=(0..g.mesh.edges.len()).collect();
            append_subset(&mut prospective,&g.mesh,&faces,&edges,g.transform,g.layer.as_deref(),g.native_document.get("material"),g.hidden)?;
            for child in &g.children {
                let mut child=child.clone();child.transform=g.transform*child.transform;
                if child.layer.is_none() {child.layer=g.layer.clone();}
                if g.hidden {child.hidden=true;}
                if let Some(paint)=g.native_document.get("material") {
                    if child.native_document.is_object() {
                        child.native_document.as_object_mut().unwrap().entry("material").or_insert_with(||paint.clone());
                    } else {child.native_document=serde_json::json!({"material":paint});}
                }
                groups.push(child);
            }
        }
        self.mesh=prospective;self.groups=groups;self.clear_selection();self.bump_version();Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    fn donut()->Mesh {
        let mut mesh=Mesh::new();
        let outer=[[0.,0.,0.],[4.,0.,0.],[4.,4.,0.],[0.,4.,0.]].map(|p|mesh.add_vertex(DVec3::from_array(p)));
        let hole=[[1.,1.,0.],[1.,2.,0.],[2.,2.,0.],[2.,1.,0.]].map(|p|mesh.add_vertex(DVec3::from_array(p)));
        let id=mesh.add_face(outer.to_vec(),vec![hole.to_vec()]).unwrap();
        mesh.faces[id].attrs.insert("future_face".into(),serde_json::json!(1));mesh.faces[id].layer=Some("paint".into());
        mesh.edges[0].attrs.insert("curve".into(),serde_json::json!({"id":"original"}));mesh.edges[0].soft=true;mesh
    }
    fn valid_incidence(mesh:&Mesh) {
        for (i,e) in mesh.edges.iter().enumerate() {
            assert!(mesh.vertices[e.v0].edges.contains(&i));assert!(mesh.vertices[e.v1].edges.contains(&i));
            for &fid in &e.faces {assert!(fid<mesh.faces.len());}
        }
        for (i,v) in mesh.vertices.iter().enumerate() {for &eid in &v.edges {let e=&mesh.edges[eid];assert!(e.v0==i||e.v1==i);}}
    }
    #[test]
    fn grouping_and_explosion_preserve_holes_attrs_and_rebuild_topology() {
        let mut scene=Scene::new();scene.mesh=donut();scene.select(SelectionItem::Face(0));
        let before=scene.clone();let id=scene.group_selection("group").unwrap();
        assert!(scene.mesh.faces.is_empty());assert!(scene.mesh.edges.is_empty());
        assert_eq!(scene.groups[id].mesh.faces[0].holes.len(),1);
        assert_eq!(scene.groups[id].mesh.faces[0].attrs,before.mesh.faces[0].attrs);
        assert_eq!(scene.groups[id].mesh.edges[0].attrs,before.mesh.edges[0].attrs);
        valid_incidence(&scene.groups[id].mesh);scene.explode_selected_groups().unwrap();
        assert_eq!(scene.mesh.faces[0].holes.len(),1);assert_eq!(scene.mesh.faces[0].attrs,before.mesh.faces[0].attrs);
        assert_eq!(scene.mesh.edges[0].attrs,before.mesh.edges[0].attrs);valid_incidence(&scene.mesh);
    }
    #[test]
    fn multi_explode_lifts_children_and_bakes_all_transforms_without_index_skips() {
        let mut scene=Scene::new();
        for n in 0..3 {
            let mut group=Group::with_mesh(format!("group{n}"),donut());
            group.transform=DMat4::from_translation(DVec3::new(0.,0.,n as f64));
            let mut child=Group::new("nested");child.transform=DMat4::from_translation(DVec3::new(5.,6.,7.));
            group.children.push(child);scene.groups.push(group);scene.select(SelectionItem::Group(n));
        }
        scene.explode_selected_groups().unwrap();assert_eq!(scene.mesh.faces.len(),3);assert_eq!(scene.groups.len(),3);
        for (n,child) in scene.groups.iter().enumerate() {assert_eq!(child.transform.w_axis.z,7.+n as f64);}
        valid_incidence(&scene.mesh);
    }
    #[test]
    fn empty_selection_and_invalid_transform_do_not_mutate_scene() {
        let mut scene=Scene::new();let before=scene.clone();assert!(scene.group_selection("no").is_err());assert_eq!(scene,before);
        let mut group=Group::with_mesh("invalid",donut());group.transform=DMat4::ZERO;scene.groups.push(group);
        scene.select(SelectionItem::Group(0));let before=scene.clone();assert!(scene.explode_selected_groups().is_err());assert_eq!(scene,before);
    }
}

#[cfg(test)]
mod source_shareback_tests {
    use super::*;
    #[test]
    fn reconstruction_drops_orphans_splits_attached_curves_and_refuses_atomically() {
        use glam::DVec3;
        let mut m = Mesh::new();
        let ids = [DVec3::ZERO, DVec3::X, DVec3::X + DVec3::Y, DVec3::Y].map(|p| m.add_vertex(p));
        m.add_face(ids.to_vec(), vec![]).unwrap();
        for e in &mut m.edges {
            e.curve_id = Some(41);
        }
        let v = m.add_vertex(DVec3::new(-1., 0., 0.));
        m.add_edge(ids[0], v).unwrap();
        let v = m.add_vertex(DVec3::new(2., 1., 0.));
        m.add_edge(ids[2], v).unwrap();
        m.add_vertex(DVec3::new(99., 99., 99.));
        m.faces[0].interior = true;
        m.faces[0].layer = Some("protected".into());
        m.faces[0].attrs.insert("vendor".into(), serde_json::json!({"opaque":[1,"Ω"]}));
        m.edges[0].hidden = true;
        m.edges[0].layer = Some("edge layer".into());
        m.edges[0].attrs.insert("vendor".into(), serde_json::json!({"nested":[2]}));
        let before = m.clone();
        let result = source_shareback_mesh(&m, DMat4::IDENTITY).unwrap();
        assert_eq!(result.vertices.len(), 6);
        assert_eq!(
            result
                .edges
                .iter()
                .filter_map(|e| e.curve_id)
                .collect::<HashSet<_>>()
                .len(),
            2
        );
        assert_eq!(m, before);
        assert_eq!(result.faces[0].history_id, m.faces[0].history_id);
        assert_eq!(result.faces[0].interior, m.faces[0].interior);
        assert_eq!(result.faces[0].layer, m.faces[0].layer);
        assert_eq!(result.faces[0].attrs, m.faces[0].attrs);
        let edge = result.edges.iter().find(|edge| edge.history_id == m.edges[0].history_id).unwrap();
        assert_eq!(edge.attrs, m.edges[0].attrs);
        assert_eq!(edge.layer, m.edges[0].layer);
        assert_eq!(edge.hidden, m.edges[0].hidden);
        for vertex in &result.vertices {
            assert!(m.vertices.iter().any(|old| old.uid == vertex.uid && old.position == vertex.position));
        }
        let mut ambiguous = m.clone();
        let mut duplicate = ambiguous.edges[0].clone();
        duplicate.attrs.insert("vendor".into(), serde_json::json!({"incompatible":true}));
        ambiguous.edges.push(duplicate);
        let held = ambiguous.clone();
        assert!(source_shareback_mesh(&ambiguous, DMat4::IDENTITY).unwrap_err().contains("incompatible native metadata"));
        assert_eq!(ambiguous, held);
        let mut malformed = m.clone();
        malformed.vertices.last_mut().unwrap().position.x = f64::INFINITY;
        assert!(source_shareback_mesh(&malformed, DMat4::IDENTITY).is_err());
        let frame = DMat4::from_rotation_y(30_f64.to_radians()) * DMat4::from_scale(DVec3::new(-2., 3., 4.));
        let precise = source_shareback_mesh(&m, frame).unwrap();
        for vertex in &precise.vertices {
            let old = m.vertices.iter().find(|old| old.uid == vertex.uid).unwrap();
            assert_eq!(vertex.position, frame.transform_point3(old.position));
        }
        assert!(source_shareback_mesh(&m, DMat4::ZERO).is_err());
        assert_eq!(m, before);
    }
}
