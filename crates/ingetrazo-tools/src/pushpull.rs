// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use glam::DVec3;
use ingetrazo_core::scene::Scene;

use crate::base::{Tool, ToolContext, ToolType};

mod inward_limit;
mod open_rebuild;

/// Interactive Push/Pull tool for extruding faces into solids and recessing geometry.
#[derive(Debug, Default, Clone)]
pub struct PushPullTool {
    pub hovered_face: Option<usize>,
    pub selected_face: Option<usize>,
    pub start_point: Option<DVec3>,
    pub extrusion_distance: f64,
    pub is_dragging: bool,
    pub last_distance: Option<f64>,
    pub keep_base: bool,
    pub error: Option<String>,
    inward_limit:Option<f64>,
    acquired_normal:Option<DVec3>,
    sweep_identity:Option<(u64,u64,bool,bool,DVec3)>,
    limit_status:Option<String>,
    acquisition_history_faces:Vec<(usize,ingetrazo_core::mesh::MeshFace)>,
}

impl PushPullTool {
    pub fn new() -> Self {
        Self::default()
    }

    fn prepare_acquisition(&mut self,scene:&mut Scene,face:usize)->Result<(),String> {
        // The source flatness probe cannot index an empty sibling ring. Refuse
        // before orientation/history changes rather than panic or discard it.
        if scene.mesh.faces.iter().any(|f| f.vertices.is_empty()) {
            return Err("Push/Pull cannot acquire a mesh containing an empty face".into());
        }
        // Cold source entry orientation is outside the extrusion command. Keep
        // only changed face rows for the dispatcher's existing history snapshot.
        let before=scene.mesh.faces.iter().map(|f|(f.vertices.clone(),f.holes.clone(),f.interior)).collect::<Vec<_>>();
        ingetrazo_core::heal::orient_acquired_component(&mut scene.mesh,face)?;
        for (id,f) in scene.mesh.faces.iter().enumerate() {
            if f.vertices!=before[id].0 || f.holes!=before[id].1 || f.interior!=before[id].2 {
                self.acquisition_history_faces.push((id,f.clone()));
            }
        }
        if !self.acquisition_history_faces.is_empty(){scene.version=scene.version.wrapping_add(1);}
        Ok(())
    }
    fn eligible_face_at(scene:&Scene,ctx:&ToolContext)->Result<Option<usize>,()> {
        // Zero edge tolerance gives the source face-only closed-group policy.
        // Placement layers decide closed-group eligibility; internal face tags
        // acquire selectability only after actual context entry.
        let group=crate::base::pick_scene_group(scene,&ctx.ray,0.);
        let loose=crate::base::pick_scene_face(scene,&ctx.ray);
        if group.is_some_and(|(_,depth)|loose.is_none_or(|(_,t,_)|depth<t)){Err(())}else{Ok(loose.map(|(id,_,_)|id))}
    }
    fn acquire_face(&mut self,scene:&Scene,ctx:&ToolContext)->Option<usize> {
        match Self::eligible_face_at(scene,ctx) {
            Ok(face)=>{self.hovered_face=face;face},
            Err(())=>{self.hovered_face=None;self.error=Some("That face belongs to a group — double-click to open it first, then Push/Pull inside".into());None}
        }
    }

    /// Original open-shell surface majority. Closed volumes and globally
    /// coplanar drawings keep their authored face normal.
    pub fn acquisition_normal(mesh:&ingetrazo_core::mesh::Mesh,id:usize)->DVec3 {
        let n=Self::source_face_normal(mesh,id);
        let flat=mesh.faces.first().is_none_or(|first| {
            let Some(&first_vertex)=first.vertices.first() else{return false;};
            let n0=mesh.face_normal(0);let p=mesh.vertices[first_vertex].position;
            mesh.faces.iter().enumerate().all(|(i,f)|f.vertices.first().is_some_and(|&v|mesh.face_normal(i).dot(n0).abs()>0.999 && (mesh.vertices[v].position-p).dot(n0).abs()<1e-3))
        });
        if mesh.is_manifold_solid()||flat{return n;}
        let rim=mesh.faces[id].vertices.iter().copied().collect::<std::collections::HashSet<_>>();
        let(mut agree,mut disagree)=(0,0);
        for &v in &mesh.faces[id].vertices {for &eid in &mesh.vertices[v].edges {
            let e=&mesh.edges[eid];if !rim.contains(&e.v0)||!rim.contains(&e.v1){continue;}
            for &other in &e.faces {if other==id{continue;}let d=mesh.face_normal(other).dot(n);if d.abs()>0.999{if d>0.{agree+=1}else{disagree+=1}}}
        }}
        if disagree>agree{-n}else{n}
    }

    fn source_face_normal(mesh:&ingetrazo_core::mesh::Mesh,id:usize)->DVec3 {
        // Source Face.normal: double Newell accumulation over Qt-stored
        // positions, then float32 area normal and Qt scaled length/division.
        // This is an authored tool direction boundary, never a document cast.
        let positions=mesh.faces[id].vertices.iter().map(|&v|mesh.vertices[v].position.as_vec3().as_dvec3()).collect::<Vec<_>>();
        let raw=ingetrazo_core::math::newell_vector(&positions).as_vec3();
        let maximum=raw.abs().max_element();
        let scaled=if maximum>0.{glam::Vec3::new(raw.x/maximum,raw.y/maximum,raw.z/maximum)}else{raw};
        let length=maximum*(scaled.x*scaled.x+scaled.y*scaled.y+scaled.z*scaled.z).sqrt();
        let n=if length<1e-9{DVec3::Z}else{glam::Vec3::new(raw.x/length,raw.y/length,raw.z/length).as_dvec3()};
        n
    }

    // Source run_stitch phase 2, restricted to the connected authored sweep.
    // Discover once, retire all redundant stations in one publication, and
    // preserve disconnected records even when they overlap the sweep's box.
    fn collapse_redundant_sweep_vertices(mesh:&mut ingetrazo_core::mesh::Mesh,seeds:impl Iterator<Item=DVec3>) {
        use std::collections::{HashSet,HashMap};
        mesh.rebuild_incidence();
        let positions=mesh.vertices.iter().enumerate().map(|(i,v)|(v.position.to_array().map(f64::to_bits),i)).collect::<HashMap<_,_>>();
        let mut owned=HashSet::new();let mut queue=Vec::new();
        for p in seeds {if let Some(&id)=positions.get(&p.to_array().map(f64::to_bits)){if owned.insert(id){queue.push(id);}}}
        let mut cursor=0;
        while cursor<queue.len(){let id=queue[cursor];cursor+=1;for &eid in &mesh.vertices[id].edges{let e=&mesh.edges[eid];let other=if e.v0==id{e.v1}else{e.v0};if owned.insert(other){queue.push(other);}}}
        let mut removed=HashSet::new();
        // Re-evaluate only neighbours after each station collapse. Ordered
        // candidates retain source's first-vertex policy without rescanning.
        let mut adjacency=mesh.vertices.iter().enumerate().map(|(id,v)|v.edges.iter().map(|&eid|{let e=&mesh.edges[eid];(if e.v0==id{e.v1}else{e.v0},e.faces.clone())}).collect::<Vec<_>>()).collect::<Vec<_>>();
        let mut candidates=queue.into_iter().collect::<std::collections::BTreeSet<_>>();
        while let Some(id)=candidates.pop_first(){
            if removed.contains(&id)||adjacency[id].len()!=2{continue;}
            let(a,faces)=&adjacency[id][0];let(b,other_faces)=&adjacency[id][1];
            if faces.is_empty()||faces!=other_faces{continue;}
            let(a,b)=(*a,*b);let p=mesh.vertices[id].position.as_vec3();
            let d0=p-mesh.vertices[a].position.as_vec3();let d1=mesh.vertices[b].position.as_vec3()-p;
            if d0.length()<1e-4||d1.length()<1e-4||d0.normalize().dot(d1.normalize())<0.999{continue;}
            let faces=faces.clone();removed.insert(id);
            adjacency[a].retain(|(v,_)|*v!=id);adjacency[b].retain(|(v,_)|*v!=id);
            if !adjacency[a].iter().any(|(v,_)|*v==b){adjacency[a].push((b,faces.clone()));adjacency[b].push((a,faces));}
            if owned.contains(&a){candidates.insert(a);}if owned.contains(&b){candidates.insert(b);}
        }
        if removed.is_empty(){return;}
        let mut joined=HashSet::new();
        // Each retired degree-two chain is walked once from its surviving
        // endpoint; its replacement edge has source constructor ownership.
        let mut visited=HashSet::new();
        for &id in &removed {
            if visited.contains(&id){continue;}
            let mut ends=Vec::new();let mut walk=vec![id];visited.insert(id);
            while let Some(v)=walk.pop(){for &eid in &mesh.vertices[v].edges{let e=&mesh.edges[eid];let other=if e.v0==v{e.v1}else{e.v0};if removed.contains(&other){if visited.insert(other){walk.push(other);}}else{ends.push(other);}}}
            if ends.len()==2{joined.insert((ends[0].min(ends[1]),ends[0].max(ends[1])));}
        }
        mesh.edges.retain(|e|!removed.contains(&e.v0)&&!removed.contains(&e.v1));
        let existing=mesh.edges.iter().map(|e|(e.v0.min(e.v1),e.v0.max(e.v1))).collect::<HashSet<_>>();
        let mut joined=joined.into_iter().collect::<Vec<_>>();joined.sort_unstable();
        for(a,b)in joined{if !existing.contains(&(a,b)){mesh.edges.push(ingetrazo_core::mesh::MeshEdge::new(a,b));}}
        let mut remap=vec![0;mesh.vertices.len()];let mut next=0;
        for(i,r)in remap.iter_mut().enumerate(){if !removed.contains(&i){*r=next;next+=1;}}
        let mut i=0;mesh.vertices.retain(|_|{let keep=!removed.contains(&i);i+=1;keep});
        for face in &mut mesh.faces {for ring in std::iter::once(&mut face.vertices).chain(&mut face.holes){ring.retain(|v|!removed.contains(v));for v in ring{*v=remap[*v];}}}
        for e in &mut mesh.edges{e.v0=remap[e.v0];e.v1=remap[e.v1];}
        mesh.rebuild_incidence();mesh.rebuild_spatial_index();
    }

    const MIN_EXTRUDE:f64=2e-4;
    /// Original attached-face inward limit: parallel overlap blockers,
    /// immediate lateral exit, then sampled nonparallel boundary crossings.
    /// Qt arithmetic is confined to this source-authored decision boundary;
    /// the document positions remain f64.
    pub fn inward_limit(mesh:&ingetrazo_core::mesh::Mesh,face_id:usize,anchor:DVec3)->Option<f64> {
        inward_limit::compute(mesh,face_id,anchor)
    }

    fn cap_limit(mesh:&ingetrazo_core::mesh::Mesh,face_id:usize)->Option<f64>{
        let face=mesh.faces.get(face_id)?;
        let anchor=(face.vertices.iter().map(|&v|mesh.vertices[v].position.as_vec3().as_dvec3()).sum::<DVec3>()/face.vertices.len()as f64).as_vec3();
        Self::inward_limit(mesh,face_id,anchor.as_dvec3())
    }
    fn limited_distance(mesh:&ingetrazo_core::mesh::Mesh,face:usize,d:f64,_keep:bool)->f64 {
        if d<0. {Self::cap_limit(mesh,face).map_or(d,|limit|d.max(-limit))}else{d}
    }

    // Source _mutate_or_top_out: only a closed-solid admission refusal
    // triggers the six cold commit trials. Each primitive is atomic; probes
    // restore their admitted model and never publish history/preview changes.
    fn commit_distance(&mut self,scene:&mut Scene,face:usize,distance:f64,keep:bool)->Result<f64,String>{
        let limit=if self.selected_face==Some(face){self.inward_limit}else{Self::cap_limit(&scene.mesh,face)};
        let clamped=if distance<0. {limit.map_or(distance,|limit|distance.max(-limit))}else{distance};
        if clamped!=distance {self.limit_status=Some(format!("Push limited to {:.2} m — deeper would leave the solid",-clamped));}
        let distance=clamped;
        let sign=if self.acquired_normal.is_some_and(|n|n.dot(scene.mesh.face_normal(face))<0.){-1.}else{1.};
        match Self::extrude_face_prepared(scene,face,distance*sign,keep,false){
            Ok(())=>Ok(distance),
            Err(error) if error.starts_with("Ctrl copy would leave an invalid solid:")
                || error.starts_with("Push/Pull cap translation would leave an invalid solid:")
                || error == "Push/Pull cap translation reached a topology collapse; use a smaller distance"
                || error == "Push/Pull would remove the whole solid" =>{
                let before=scene.mesh.clone();let selection=scene.selection.clone();let order=scene.selection_order.clone();let (version,view_version)=(scene.version,scene.view_version);let(mut low,mut high)=(0.,1.);let mut best=None;
                for _ in 0..6 {let mid=(low+high)*0.5;let d=distance*mid;
                    if d.abs()>=Self::MIN_EXTRUDE&&Self::extrude_face_prepared(scene,face,d*sign,keep,false).is_ok(){best=Some(d);low=mid;}else{high=mid;}
                    scene.mesh=before.clone();scene.selection=selection.clone();scene.selection_order=order.clone();scene.version=version;scene.view_version=view_version;
                }
                let Some(d)=best else{return Err(error);};Self::extrude_face_prepared(scene,face,d*sign,keep,false)?;
                self.limit_status=Some(format!("Push stopped at {:.2} m: going further would break the solid",d.abs()));Ok(d)
            },
            Err(error)=>Err(error),
        }
    }

    // Source run_stitch/dedupe_faces + flat heal: retain subdivision
    // cycles, including an emptied mother represented by holes, rather than
    // replacing the whole surface by a single cap or a Boolean region.
    fn collapse_surface(translated:&ingetrazo_core::mesh::Mesh,faces:&std::collections::HashSet<usize>)->Result<ingetrazo_core::mesh::Mesh,String> {
        use ingetrazo_core::{mesh::Mesh,math::{newell_vector,plane_basis}};
        let mut out=Mesh::new();let mut cycles=std::collections::HashMap::new();
        for id in (0..translated.faces.len()).filter(|id|faces.contains(id)) {
            let source=&translated.faces[id];let mut loops=vec![];
            for ring in std::iter::once(&source.vertices).chain(&source.holes) {
                let mut ids=ring.iter().map(|&v|out.add_vertex(translated.vertices[v].position)).collect::<Vec<_>>();
                ids.dedup();if ids.len()>1&&ids.first()==ids.last(){ids.pop();}
                loops.push(ids);
            }
            if loops[0].iter().copied().collect::<std::collections::HashSet<_>>().len()<3 {continue;}
            if loops.iter().any(|r|r.len()<3||r.iter().copied().collect::<std::collections::HashSet<_>>().len()!=r.len()) {return Err("Collapse would leave an invalid repeated-vertex surface; document unchanged".into());}
            if newell_vector(&loops[0].iter().map(|&v|out.vertices[v].position).collect::<Vec<_>>()).length()<1e-9 {return Err("Collapse would leave a degenerate supporting face; document unchanged".into());}
            let mut key=(0..loops[0].len()).map(|i|{let a=loops[0][i];let b=loops[0][(i+1)%loops[0].len()];(a.min(b),a.max(b))}).collect::<Vec<_>>();key.sort_unstable();key.dedup();
            let mut face=source.clone();face.vertices=loops.remove(0);face.holes=loops;face.interior=false;
            if let Some(&old)=cycles.get(&key) {
                let previous:&mut ingetrazo_core::mesh::MeshFace=&mut out.faces[old];
                if face.holes.len()>previous.holes.len(){if face.attrs.is_empty(){face.attrs=previous.attrs.clone();face.layer=previous.layer.clone();}*previous=face;}
                else if previous.attrs.is_empty(){previous.attrs=face.attrs;previous.layer=face.layer;}
            }else{let id=out.faces.len();out.faces.push(face);cycles.insert(key,id);}
        }
        if out.faces.is_empty(){return Err("Collapse has no surface boundary".into());}
        // Source uses outer Newell area (holes are not subtracted) and
        // float32 weighted-normal accumulation for flat winding consensus.
        let areas=out.faces.iter().map(|f|newell_vector(&f.vertices.iter().map(|&v|out.vertices[v].position).collect::<Vec<_>>()).as_vec3().length()as f64*0.5).collect::<Vec<_>>();
        let normals=(0..out.faces.len()).map(|id|out.face_normal(id)).collect::<Vec<_>>();
        let dominant=normals.iter().zip(&areas).fold(glam::Vec3::ZERO,|sum,(n,a)|sum+n.as_vec3()*(*a as f32));
        for (id,face) in out.faces.iter_mut().enumerate(){if normals[id].as_vec3().dot(dominant)<0.{face.vertices.reverse();}}
        if out.faces.len()>3000 {return Self::relink_surface(&out);}
        let n=normals.iter().copied().find(|n|n.length_squared()>0.5).ok_or("Collapse has no nondegenerate supporting plane")?;
        let (u,w)=plane_basis(n);let project=|p:DVec3|glam::DVec2::new(p.dot(u),p.dot(w));
        let rings=out.faces.iter().map(|f|std::iter::once(&f.vertices).chain(&f.holes).map(|r|r.iter().map(|&v|project(out.vertices[v].position)).collect::<Vec<_>>()).collect::<Vec<_>>()).collect::<Vec<_>>();
        let inside=|p:glam::DVec2,r:&[glam::DVec2],strict:bool| {
            let mut inside=false;for i in 0..r.len(){let a=r[i];let b=r[(i+1)%r.len()];let d=b-a;let rel=p-a;let dot=rel.dot(d);if strict&&(d.x*rel.y-d.y*rel.x).abs()<=1e-7&&dot>=-1e-7&&dot<=d.length_squared()+1e-7{return false;}
                if (a.y>p.y)!=(b.y>p.y)&&p.x<a.x+(b.x-a.x)*(p.y-a.y)/(b.y-a.y){inside=!inside;}}
            inside
        };
        let hole_probes=out.faces.iter().map(|face| {
            let points=face.vertices.iter().map(|&v|out.vertices[v].position).collect::<Vec<_>>();
            let mean=project((points.iter().copied().sum::<DVec3>()/points.len()as f64).as_vec3().as_dvec3());
            let mut best=None;let mut area=0.;
            for (a,b,c) in ingetrazo_core::triangulate::triangulate(&points,&[],Some(n)) {
                let next=(b-a).as_vec3().cross((c-a).as_vec3()).length_squared();
                if next>area {area=next;best=Some(project(((a.as_vec3()+b.as_vec3()+c.as_vec3())/3.).as_dvec3()));}
            }
            (mean,best)
        }).collect::<Vec<_>>();
        // _loop_inside_loop retries the largest outer triangle when a
        // concave loop's vertex mean lies outside the prospective hole.
        let in_hole=|i:usize,j:usize|rings[j][1..].iter().any(|hole|inside(hole_probes[i].0,hole,false)||hole_probes[i].1.is_some_and(|p|inside(p,hole,false)));
        let mut removed=vec![false;out.faces.len()];
        for i in 0..out.faces.len(){if areas[i]<1e-8{continue;}let covered=(0..out.faces.len()).filter(|&j|i!=j&&!removed[j]&&areas[j]<areas[i]&&rings[j][0].iter().all(|&p|inside(p,&rings[i][0],true))&&!in_hole(j,i)).map(|j|areas[j]).sum::<f64>();if covered>0.5*areas[i]{removed[i]=true;}}
        let mut punch=vec![Vec::new();out.faces.len()];
        for i in 0..out.faces.len(){if removed[i]{continue;}let tris=out.triangulate_face(i);if tris.is_empty(){continue;}
            for j in 0..out.faces.len(){if i==j||removed[j]||areas[j]<=areas[i]||in_hole(i,j){continue;}
                let count=tris.iter().filter(|&&(a,b,c)|{let p=project((a+b+c)/3.);inside(p,&rings[j][0],false)&&!rings[j][1..].iter().any(|hole|inside(p,hole,false))}).count();
                if count*2>tris.len(){punch[j].push(i);break;}}
        }
        let snapshot=out.faces.clone();out.faces.clear();
        for (id,face) in snapshot.iter().enumerate(){if !removed[id]&&punch[id].is_empty(){out.faces.push(face.clone());}}
        for (id,face) in snapshot.iter().enumerate(){if !removed[id]&&!punch[id].is_empty(){let mut face=face.clone();face.history_id=ingetrazo_core::mesh::MeshFace::new(Vec::new()).history_id;for &small in &punch[id]{face.holes.push(snapshot[small].vertices.clone());}out.faces.push(face);}}
        Self::relink_surface(&out)
    }
    fn stitch_open_sweep(source:&ingetrazo_core::mesh::Mesh,seeds:impl Iterator<Item=DVec3>)->Result<ingetrazo_core::mesh::Mesh,String>{
        let mut lo=DVec3::splat(f64::INFINITY);let mut hi=DVec3::splat(f64::NEG_INFINITY);
        for p in seeds {lo=lo.min(p);hi=hi.max(p);}
        // Cold sweep refinement: index vertices once, then search only each
        // intersecting edge's coordinate interval instead of E×V scans.
        let mut by_x=(0..source.vertices.len()).collect::<Vec<_>>();
        by_x.sort_unstable_by(|&a,&b|source.vertices[a].position.x.total_cmp(&source.vertices[b].position.x));
        let mut out=ingetrazo_core::mesh::Mesh::new();
        for v in &source.vertices{let id=out.add_vertex(v.position);out.vertices[id].uid=v.uid;}
        for f in &source.faces {
            let refine=|ring:&Vec<usize>| {let mut refined=vec![];for i in 0..ring.len(){
                let a=source.vertices[ring[i]].position;let b=source.vertices[ring[(i+1)%ring.len()]].position;let d=b-a;
                let mut stations=vec![(0.,ring[i])];
                if a.max(b).cmplt(lo-DVec3::splat(1e-6)).any()==false && a.min(b).cmpgt(hi+DVec3::splat(1e-6)).any()==false {
                    let lower=by_x.partition_point(|&id|source.vertices[id].position.x<a.x.min(b.x)-1e-6);
                    let upper=by_x.partition_point(|&id|source.vertices[id].position.x<=a.x.max(b.x)+1e-6);
                    for &id in &by_x[lower..upper]{let p=source.vertices[id].position;if p.cmplt(a.min(b)-DVec3::splat(1e-6)).any()||p.cmpgt(a.max(b)+DVec3::splat(1e-6)).any(){continue;}let t=(p-a).dot(d)/d.length_squared();if t>1e-7&&t<1.-1e-7&&(p-a-d*t).length()<1e-6{stations.push((t,id));}}
                }
                stations.sort_by(|a,b|a.0.total_cmp(&b.0));stations.dedup_by(|a,b|(a.0-b.0).abs()<1e-7);refined.extend(stations.into_iter().map(|(_,id)|id));
            }refined};
            let vertices=refine(&f.vertices);let holes=f.holes.iter().map(refine).collect();let id=out.add_face(vertices,holes).ok_or("Invalid open sweep support")?;
            let vertices=out.faces[id].vertices.clone();let holes=out.faces[id].holes.clone();out.faces[id]=f.clone();out.faces[id].vertices=vertices;out.faces[id].holes=holes;
        }
        Ok(out)
    }

    fn weld_translated_surface(source:&ingetrazo_core::mesh::Mesh,scope:&std::collections::HashSet<usize>)->Result<ingetrazo_core::mesh::Mesh,String>{
        use ingetrazo_core::{mesh::Mesh,math::newell_vector};
        let mut out=Mesh::new();let mut remap=Vec::with_capacity(source.vertices.len());
        for v in &source.vertices {let before=out.vertices.len();let id=out.add_vertex(v.position);if id==before{out.vertices[id].uid=v.uid;}remap.push(id);}
        let mut cycles=std::collections::HashMap::new();
        for (id,f) in source.faces.iter().enumerate() {
            if !scope.contains(&id){continue;}
            let mut rings=std::iter::once(&f.vertices).chain(&f.holes).map(|r|{let mut ids=r.iter().map(|&v|remap[v]).collect::<Vec<_>>();ids.dedup();if ids.len()>1&&ids.first()==ids.last(){ids.pop();}ids}).collect::<Vec<_>>();
            if rings[0].iter().copied().collect::<std::collections::HashSet<_>>().len()<3 {continue;}
            if rings.iter().any(|r|r.len()<3||r.iter().copied().collect::<std::collections::HashSet<_>>().len()!=r.len()){return Err("Translated endpoint leaves an invalid repeated-vertex surface; document unchanged".into());}
            if newell_vector(&rings[0].iter().map(|&v|out.vertices[v].position).collect::<Vec<_>>()).length()<1e-9{return Err("Translated endpoint leaves a degenerate surface; document unchanged".into());}
            let mut key=(0..rings[0].len()).map(|i|{let a=rings[0][i];let b=rings[0][(i+1)%rings[0].len()];(a.min(b),a.max(b))}).collect::<Vec<_>>();key.sort_unstable();key.dedup();
            let mut face=f.clone();face.vertices=rings.remove(0);face.holes=rings;
            if let Some(&old)=cycles.get(&key){let previous:&mut ingetrazo_core::mesh::MeshFace=&mut out.faces[old];if face.holes.len()>previous.holes.len(){if face.attrs.is_empty(){face.attrs=previous.attrs.clone();face.layer=previous.layer.clone();}*previous=face;}else if previous.attrs.is_empty(){previous.attrs=face.attrs;previous.layer=face.layer;}}
            else{let id=out.add_face(face.vertices.clone(),face.holes.clone()).ok_or("Invalid translated endpoint surface")?;out.faces[id]=face;cycles.insert(key,id);}
        }
        // Duplicate replacement may change hole incidence; refresh without
        // replacing surviving rows or runtime entity identities.
        out.rebuild_incidence();Ok(out)
    }

    fn relink_surface(source:&ingetrazo_core::mesh::Mesh)->Result<ingetrazo_core::mesh::Mesh,String> {
        let mut out=ingetrazo_core::mesh::Mesh::new();for vertex in &source.vertices{out.add_vertex(vertex.position);}
        for face in &source.faces{let id=out.add_face(face.vertices.clone(),face.holes.clone()).ok_or("Invalid collapsed surface")?;out.faces[id]=face.clone();}
        Ok(out)
    }

    pub fn reset(&mut self) {
        self.hovered_face = None;
        self.selected_face = None;
        self.start_point = None;
        self.inward_limit=None;
        self.acquired_normal=None;
        self.sweep_identity=None;
        self.extrusion_distance = 0.0;
        self.is_dragging = false;
        self.keep_base = false;
        self.error = None;
        self.limit_status=None;
        self.acquisition_history_faces.clear();
    }
    fn reset_after_commit(&mut self){let status=self.limit_status.take();let acquisition=std::mem::take(&mut self.acquisition_history_faces);self.reset();self.limit_status=status;self.acquisition_history_faces=acquisition;}

    /// Prepare the whole extrusion before changing the scene. A free face
    /// becomes a closed prism; a bordered surface loses its old base. Closed
    /// solids use union/difference so inward pushes remove material and outward
    /// pushes remove the former cap/internal partitions.
    pub fn extrude_face(scene: &mut Scene, face_id: usize, distance: f64) -> Result<(), String> {
        Self::extrude_face_with_options(scene, face_id, distance, false)
    }
    /// Ctrl copies a segment and retains the original cap as a slab division.
    // A host-hole touching its outer rim can have redundant polygon-edge
    // incidences while its actual triangulated boundary is a proper solid.
    // Admit only a fully checked oriented triangle manifold, never the raw
    // source's weaker polygon closure test.
    fn validate_triangle_boundary(mesh: &ingetrazo_core::mesh::Mesh) -> Result<(), String> {
        let mut boundary = ingetrazo_core::mesh::Mesh::new();
        for (id, face) in mesh.faces.iter().enumerate() {
            if face.interior { continue; }
            let triangles = mesh.triangulate_face(id);
            if triangles.is_empty() { return Err("Solid face has no triangle boundary".into()); }
            for (a,b,c) in triangles {
                if !a.is_finite() || !b.is_finite() || !c.is_finite() {
                    return Err("Solid triangle coordinates must be finite".into());
                }
                if (b-a).cross(c-a).length_squared() == 0.0 { continue; }
                let ids = [a,b,c].into_iter().map(|p|boundary.add_vertex(p)).collect();
                boundary.add_face(ids,vec![]).ok_or("Invalid solid boundary triangle")?;
            }
        }
        ingetrazo_core::solids::validate_solid_mesh(&boundary)
    }

    pub fn extrude_face_with_options(
        scene: &mut Scene,
        face_id: usize,
        distance: f64,
        keep_base: bool,
    ) -> Result<(), String> {
        Self::extrude_face_prepared(scene,face_id,distance,keep_base,true)
    }
    fn extrude_face_prepared(scene:&mut Scene,face_id:usize,distance:f64,keep_base:bool,compute_limit:bool)->Result<(),String>{
        use ingetrazo_core::{
            math::newell_vector,
            mesh::{Mesh, MeshFace},
            solids::{perform_solid_op, solid_report, SolidOp},
        };
        let mesh = &scene.mesh;
        let face = mesh.faces.get(face_id).ok_or("Invalid face ID")?;
        if mesh.vertices.iter().any(|v| !v.position.is_finite())
            || mesh.faces.iter().any(|f| {
                f.vertices
                    .iter()
                    .chain(f.holes.iter().flatten())
                    .any(|&v| v >= mesh.vertices.len())
            })
            || mesh.edges.iter().any(|e| {
                e.v0 >= mesh.vertices.len()
                    || e.v1 >= mesh.vertices.len()
                    || e.faces.iter().any(|&f| f >= mesh.faces.len())
            })
        {
            return Err("Document mesh has invalid references or coordinates".into());
        }
        if !distance.is_finite() {
            return Err("Distance must be finite".into());
        }
        if face.vertices.len() < 3
            || face.holes.iter().any(|h| h.len() < 3)
            || face
                .vertices
                .iter()
                .chain(face.holes.iter().flatten())
                .any(|&v| v >= mesh.vertices.len() || !mesh.vertices[v].position.is_finite())
        {
            return Err("Invalid face boundary".into());
        }
        if distance.abs() < Self::MIN_EXTRUDE {
            return Ok(());
        }
        let distance=if compute_limit {Self::limited_distance(mesh,face_id,distance,keep_base)}else{distance};
        if distance.abs()<Self::MIN_EXTRUDE {return Ok(());}
        let normal = mesh.face_normal(face_id);
        let origin = mesh.vertices[face.vertices[0]].position;
        if face
            .vertices
            .iter()
            .chain(face.holes.iter().flatten())
            .any(|&v| (mesh.vertices[v].position - origin).dot(normal).abs() > 1e-7)
        {
            return Err("Push/Pull requires a planar face".into());
        }
        let outer: Vec<_> = face
            .vertices
            .iter()
            .map(|&v| mesh.vertices[v].position)
            .collect();
        if newell_vector(&outer).length_squared() < 1e-16 {
            return Err("Push/Pull requires a nondegenerate face".into());
        }
        let mut rings = vec![outer];
        for hole in &face.holes {
            let mut positions: Vec<_> = hole.iter().map(|&v| mesh.vertices[v].position).collect();
            // Outer and hole loops must have opposite winding.
            if newell_vector(&positions).dot(normal) > 0.0 {
                positions.reverse();
            }
            rings.push(positions);
        }
        // QVector3D multiplies and adds in float32 on the operative source
        // authoring path. Restrict that compatibility boundary to generated
        // and moved extrusion vertices; untouched document geometry stays f64.
        let translation=(Self::source_face_normal(mesh,face_id).as_vec3()*(distance as f32)).as_dvec3();
        if !translation.is_finite(){return Err("Push/Pull distance exceeds the supported authoring range".into());}
        // Source _paint_of: side walls own paint in their own planes, while
        // the cap remains the full metadata continuation of the base.
        let mut wall_source=face.clone();
        wall_source.layer=None;
        wall_source.attrs.retain(|key,_|matches!(key.as_str(),"color"|"mat"|"texture"|"opacity"|"ifc"));
        if let Some(tag)=wall_source.attrs.get("ifc") {
            let truth=match tag {serde_json::Value::Null=>false,serde_json::Value::Bool(b)=>*b,serde_json::Value::Number(n)=>n.as_f64()!=Some(0.),serde_json::Value::String(v)=>!v.is_empty(),serde_json::Value::Array(v)=>!v.is_empty(),serde_json::Value::Object(v)=>!v.is_empty()};
            if !truth {wall_source.attrs.remove("ifc");} else if !tag.is_object(){return Err("Push/Pull cannot extend an unsupported non-object IFC tag; metadata left unchanged".into());}
        }
        if let Some(texture)=wall_source.attrs.get_mut("texture").and_then(|v|v.as_object_mut()) {texture.remove("uvw");texture.insert("planar".into(),serde_json::json!(true));}

        fn add_polygon(
            mesh: &mut Mesh,
            outer: &[DVec3],
            holes: &[Vec<DVec3>],
            source: &MeshFace,
        ) -> Result<(), String> {
            let ids: Vec<_> = outer.iter().map(|&p| mesh.add_vertex(p)).collect();
            let hole_ids: Vec<Vec<_>> = holes
                .iter()
                .map(|h| h.iter().map(|&p| mesh.add_vertex(p)).collect())
                .collect();
            if ids
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != ids.len()
                || hole_ids.iter().any(|h| {
                    h.iter()
                        .copied()
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        != h.len()
                })
            {
                return Err("Extrusion is below the document welding tolerance".into());
            }
            let id = mesh
                .add_face(ids, hole_ids)
                .ok_or("Invalid extrusion face")?;
            mesh.faces[id].attrs = source.attrs.clone();
            mesh.faces[id].layer = source.layer.clone();
            Ok(())
        }
        let shifted: Vec<Vec<_>> = rings
            .iter()
            .map(|r| r.iter().map(|&p|(p.as_vec3()+translation.as_vec3()).as_dvec3()).collect())
            .collect();
        let mut prism = Mesh::new();
        let mut bottom = rings.clone();
        let mut top = shifted.clone();
        if distance > 0.0 {
            for ring in &mut bottom {
                ring.reverse();
            }
        } else {
            for ring in &mut top {
                ring.reverse();
            }
        }
        add_polygon(&mut prism, &bottom[0], &bottom[1..], face)?;
        add_polygon(&mut prism, &top[0], &top[1..], face)?;
        for (base, cap) in rings.iter().zip(&shifted) {
            for i in 0..base.len() {
                let j = (i + 1) % base.len();
                let mut wall = vec![base[i], base[j], cap[j], cap[i]];
                if distance < 0.0 {
                    wall.reverse();
                }
                add_polygon(&mut prism, &wall, &[], &wall_source)?;
            }
        }
        if !solid_report(&prism).0 {
            return Err("Extrusion did not form a closed prism".into());
        }
        // Treat the selected connected face component independently so loose
        // construction edges or another object cannot disable the solid guard.
        // Derive connectivity from authoritative face rings, not cached edge
        // incidence (public face holes may have been added by an editing tool).
        let mut adjacent = std::collections::HashMap::<(usize, usize), Vec<usize>>::new();
        for (fid, original) in mesh.faces.iter().enumerate() {
            for ring in std::iter::once(&original.vertices).chain(&original.holes) {
                for i in 0..ring.len() {
                    let a = ring[i];
                    let b = ring[(i + 1) % ring.len()];
                    adjacent.entry((a.min(b), a.max(b))).or_default().push(fid);
                }
            }
        }
        let mut component_faces = std::collections::HashSet::from([face_id]);
        let mut pending = vec![face_id];
        while let Some(fid) = pending.pop() {
            let original = &mesh.faces[fid];
            for ring in std::iter::once(&original.vertices).chain(&original.holes) {
                for i in 0..ring.len() {
                    let a = ring[i];
                    let b = ring[(i + 1) % ring.len()];
                    for &neighbor in &adjacent[&(a.min(b), a.max(b))] {
                        if component_faces.insert(neighbor) {
                            pending.push(neighbor);
                        }
                    }
                }
            }
        }
        let mut component = Mesh::new();
        for (fid, original) in mesh.faces.iter().enumerate() {
            if !component_faces.contains(&fid) {
                continue;
            }
            let points: Vec<_> = original
                .vertices
                .iter()
                .map(|&v| mesh.vertices[v].position)
                .collect();
            let holes: Vec<Vec<_>> = original
                .holes
                .iter()
                .map(|h| h.iter().map(|&v| mesh.vertices[v].position).collect())
                .collect();
            add_polygon(&mut component, &points, &holes, original)?;
            component.faces.last_mut().unwrap().interior = original.interior;
        }
        let was_solid = solid_report(&component).0
            || (component.faces.iter().any(|f| !f.holes.is_empty())
                && Self::validate_triangle_boundary(&component).is_ok());
        // Original _classify_base admits a clean cap translation only when
        // every rim edge backs onto a perpendicular wall swept along the push.
        // Moving shared cap vertices extends those walls instead of creating
        // provenance seams between a copied belt and the original wall.
        let prism_cap = std::iter::once(&face.vertices).chain(&face.holes).all(|ring| {
            (0..ring.len()).all(|i| {
                let a=ring[i];let b=ring[(i+1)%ring.len()];
                let neighbors=&adjacent[&(a.min(b),a.max(b))];
                neighbors.len()==2 && neighbors.iter().copied().filter(|&id|id!=face_id).all(|id| {
                    let other=&mesh.faces[id];
                    mesh.face_normal(id).dot(normal).abs()<0.001 && (0..other.vertices.len()).any(|i| {
                        let edge=mesh.vertices[other.vertices[(i+1)%other.vertices.len()]].position-mesh.vertices[other.vertices[i]].position;
                        edge.length()>1e-9 && edge.normalize().dot(normal).abs()>0.999
                    })
                })
            })
        });
        let mut ctrl_rebuilt_planes = Vec::new();
        let mut collapsed_positions=None;
        let mut candidate = if prism_cap && was_solid && !keep_base {
            let mut result=mesh.clone();
            let moving=face.vertices.iter().chain(face.holes.iter().flatten()).copied().collect::<std::collections::HashSet<_>>();
            for &id in &moving {result.vertices[id].position=(result.vertices[id].position.as_vec3()+translation.as_vec3()).as_dvec3();}
            let end=origin+translation;
            let flat=component_faces.iter().all(|&id|result.faces[id].vertices.iter().chain(result.faces[id].holes.iter().flatten()).all(|&v|(result.vertices[v].position-end).dot(normal).abs()<1e-6));
            if flat {
                let mut collapsed=Self::collapse_surface(&result,&component_faces)?;
                collapsed_positions=Some(moving.iter().map(|&v|(v,result.vertices[v].position)).collect::<std::collections::HashMap<_,_>>());
                for (id,other) in mesh.faces.iter().enumerate() {
                    if component_faces.contains(&id){continue;}
                    let points=other.vertices.iter().map(|&v|mesh.vertices[v].position).collect::<Vec<_>>();
                    let holes=other.holes.iter().map(|ring|ring.iter().map(|&v|mesh.vertices[v].position).collect()).collect::<Vec<Vec<_>>>();
                    add_polygon(&mut collapsed,&points,&holes,other)?;
                    collapsed.faces.last_mut().unwrap().interior=other.interior;
                }
                collapsed
            }else{
            if component_faces.iter().any(|&id| result.face_area(id)<1e-12
                || std::iter::once(&result.faces[id].vertices).chain(&result.faces[id].holes).any(|ring|
                    (0..ring.len()).any(|i| result.vertices[ring[i]].position.distance_squared(
                        result.vertices[ring[(i+1)%ring.len()]].position)<1e-12))) {
                // Landing a raised cap flush consumes its side walls. Repair
                // the translated boundary with source weld and affected-plane repair,
                // rather than rejecting a legitimate endpoint or retaining fins.
                // Source prism path translates existing shared cap vertices,
                // welds coincident endpoints/removes slivers, then classifies
                // affected planes. A swept subtraction is wrong for an outward
                // annulus landing on an already-raised neighbour.
                let mut repaired=Self::weld_translated_surface(&result,&component_faces)?;
                repaired=ingetrazo_core::planar::stitch_boundaries(&repaired)?;
                let seeds=face.vertices.iter().chain(face.holes.iter().flatten()).flat_map(|&v|[ingetrazo_core::math::PointKey::from_vec3(mesh.vertices[v].position),ingetrazo_core::math::PointKey::from_vec3(result.vertices[v].position)]).collect::<std::collections::HashSet<_>>();
                let marked=repaired.faces.iter().filter(|f|f.vertices.iter().any(|&v|seeds.contains(&ingetrazo_core::math::PointKey::from_vec3(repaired.vertices[v].position)))).map(|f|f.history_id).collect();
                repaired=open_rebuild::rebuild_marked(&repaired,mesh,&component_faces,distance<0.,&marked)?;
                collapsed_positions=Some(moving.iter().map(|&v|(v,result.vertices[v].position)).collect());
                if repaired.faces.is_empty(){return Err("Endpoint has no valid surface boundary".into());}
                // A formerly internal retained slab can become the new outer
                // cap after welding. Source reclassifies it on prism exit.
                ingetrazo_core::heal::orient_boundary_candidate(&mut repaired)?;
                Self::validate_triangle_boundary(&repaired).map_err(|e|format!("Prism endpoint triangle admission: {e}"))?;
                for (id,other) in mesh.faces.iter().enumerate() {
                    if component_faces.contains(&id){continue;}
                    let points=other.vertices.iter().map(|&v|mesh.vertices[v].position).collect::<Vec<_>>();
                    let holes=other.holes.iter().map(|ring|ring.iter().map(|&v|mesh.vertices[v].position).collect()).collect::<Vec<Vec<_>>>();
                    add_polygon(&mut repaired,&points,&holes,other)?;repaired.faces.last_mut().unwrap().interior=other.interior;
                }
                repaired
            }else{
            result=ingetrazo_core::planar::stitch_boundaries(&result)?;
            let ids=component_faces.iter().copied().collect::<Vec<_>>();
            // Topology is unchanged before stitch; a collapse can consume
            // faces, so validate the full changed component by geometry below.
            if result.faces.len()==mesh.faces.len(){
                let body=ingetrazo_core::pieces::copy_part(&result,&ingetrazo_core::pieces::Piece{faces:ids,edges:vec![]})?;
                Self::validate_triangle_boundary(&body).map_err(|e|format!("Push/Pull cap translation would leave an invalid solid: {e}"))?;
            }else{return Err("Push/Pull cap translation reached a topology collapse; use a smaller distance".into());}
            result
            }
            }
        } else if keep_base && component_faces.len() > 1 {
            if was_solid {
                ingetrazo_core::solids::validate_solid_mesh(&component)?;
            }
            ingetrazo_core::solids::validate_solid_mesh(&prism)?;
            let mut result = mesh.clone();
            let mut wall_planes = vec![];
            for (id, original) in prism.faces.iter().enumerate() {
                if id == 0 {
                    continue;
                }
                let points: Vec<_> = original
                    .vertices
                    .iter()
                    .map(|&v| prism.vertices[v].position)
                    .collect();
                let holes: Vec<Vec<_>> = original
                    .holes
                    .iter()
                    .map(|r| r.iter().map(|&v| prism.vertices[v].position).collect())
                    .collect();
                if id >= 2 {
                    wall_planes.push((points[0], prism.face_normal(id)));
                }
                add_polygon(&mut result, &points, &holes, original)?;
            }
            // Coplanar inward strips are overlapping boundary subdivisions,
            // not coincident walls. Rebuild each touched wall plane while the
            // kept slab and its belt remain explicit document geometry.
            for (point, normal) in wall_planes {
                let selected = result.faces.iter().position(|f| {
                    f.vertices
                        .iter()
                        .all(|&v| (result.vertices[v].position - point).dot(normal).abs() < 1e-6)
                });
                if let Some(id) = selected {
                    result = ingetrazo_core::planar::rebuild_faces(&result, &[id])?;
                    // Source cap_rebuild::plane_key canonicalizes this plane;
                    // rebuilt partitions keep its arrangement winding.
                    let axis = if normal.x.abs() >= normal.y.abs() && normal.x.abs() >= normal.z.abs() { 0 }
                        else if normal.y.abs() >= normal.z.abs() { 1 } else { 2 };
                    let canonical = if normal[axis] < 0.0 { -normal } else { normal };
                    ctrl_rebuilt_planes.push((point, canonical));
                }
            }
            result = ingetrazo_core::planar::stitch_boundaries(&result)?;
            ingetrazo_core::heal::orient_faces(&result)?
        } else if was_solid {
            let anchor = component.vertices[0].position;
            let signed_volume: f64 = (0..component.faces.len())
                .flat_map(|fid| component.triangulate_face(fid))
                .map(|(a, b, c)| (a - anchor).dot((b - anchor).cross(c - anchor)) / 6.0)
                .sum();
            // Direction is physical, independent of globally reversed input winding.
            let op = if distance * signed_volume > 0.0 {
                SolidOp::Union
            } else {
                SolidOp::Subtract
            };
            let mut result = perform_solid_op(op, &component, &prism)?;
            if result.faces.is_empty() {
                return Err("Push/Pull would remove the whole solid".into());
            }
            for (fid, original) in mesh.faces.iter().enumerate() {
                if component_faces.contains(&fid) {
                    continue;
                }
                let points: Vec<_> = original
                    .vertices
                    .iter()
                    .map(|&v| mesh.vertices[v].position)
                    .collect();
                let holes: Vec<Vec<_>> = original
                    .holes
                    .iter()
                    .map(|h| h.iter().map(|&v| mesh.vertices[v].position).collect())
                    .collect();
                add_polygon(&mut result, &points, &holes, original)?;
            }
            result
        } else {
            // An embedded sheet face is consumed in either direction. All
            // unrelated faces and loose edges remain part of the document.
            let attached = std::iter::once(&face.vertices)
                .chain(&face.holes)
                .any(|ring| {
                    (0..ring.len()).any(|i| {
                        let a = ring[i];
                        let b = ring[(i + 1) % ring.len()];
                        adjacent[&(a.min(b), a.max(b))].len() > 1
                    })
                });
            let mut result = Mesh::new();
            for (id, original) in mesh.faces.iter().enumerate() {
                if id == face_id {
                    continue;
                }
                let points: Vec<_> = original
                    .vertices
                    .iter()
                    .map(|&v| mesh.vertices[v].position)
                    .collect();
                let holes: Vec<Vec<_>> = original
                    .holes
                    .iter()
                    .map(|h| h.iter().map(|&v| mesh.vertices[v].position).collect())
                    .collect();
                add_polygon(&mut result, &points, &holes, original)?;
                result.faces.last_mut().unwrap().history_id=original.history_id;
            }
            if attached {
                add_polygon(&mut result, &shifted[0], &shifted[1..], face)?;
                for (base, cap) in rings.iter().zip(&shifted) {
                    for i in 0..base.len() {
                        let j = (i + 1) % base.len();
                        add_polygon(&mut result, &[base[i], base[j], cap[j], cap[i]], &[], &wall_source)?;
                    }
                }
            } else {
                let first = result.faces.len();
                for original in &prism.faces {
                    let points: Vec<_> = original
                        .vertices
                        .iter()
                        .map(|&v| prism.vertices[v].position)
                        .collect();
                    let holes: Vec<Vec<_>> = original
                        .holes
                        .iter()
                        .map(|h| h.iter().map(|&v| prism.vertices[v].position).collect())
                        .collect();
                    add_polygon(&mut result, &points, &holes, original)?;
                }
                // Original free authoring keeps the translated cap and wall
                // cycles for either sign. Its final orient pass flips these
                // only when the complete target mesh is closed; a disconnected
                // open sheet leaves the authored inward cycles intact.
                if distance < 0.0 && !solid_report(&result).0 {
                    for authored in &mut result.faces[first + 1..] {
                        authored.vertices.reverse();
                        for hole in &mut authored.holes { hole.reverse(); }
                    }
                }
            }
            result
        };
        // Original run_stitch refines the supporting open surface at the
        // consumed face rim before publication (including hole rims).
        if !was_solid {
            candidate=Self::stitch_open_sweep(&candidate,rings.iter().flatten().chain(shifted.iter().flatten()).copied())?;
        }
        // Source sheet_fast excludes a fully attached perpendicular support.
        // Its open component then uses the same affected-plane boundary rebuild
        // as a solid; disconnected authored geometry stays outside the domain.
        let attached_all=std::iter::once(&face.vertices).chain(&face.holes).all(|ring|(0..ring.len()).all(|i|adjacent[&(ring[i].min(ring[(i+1)%ring.len()]),ring[i].max(ring[(i+1)%ring.len()]))].len()>1));
        let perpendicular_support=std::iter::once(&face.vertices).chain(&face.holes).any(|ring|(0..ring.len()).any(|i|adjacent[&(ring[i].min(ring[(i+1)%ring.len()]),ring[i].max(ring[(i+1)%ring.len()]))].iter().any(|&other|other!=face_id&&mesh.face_normal(other).dot(normal).abs()<0.999)));
        if !was_solid&&!keep_base&&attached_all&&perpendicular_support {
            candidate=open_rebuild::rebuild(&candidate,mesh,&component_faces,distance<0.)?;
        }
        if was_solid && !keep_base && !prism_cap {
            let mut rims=vec![];
            for (base,cap) in rings.iter().zip(&shifted){for i in 0..base.len(){let j=(i+1)%base.len();rims.push((base[i],base[j]));rims.push((cap[i],cap[j]));rims.push((base[i],cap[i]));}}
            candidate=ingetrazo_core::solids::dissolve_rim_seams(&candidate,&rims)?;
            candidate=open_rebuild::rebuild(&candidate,mesh,&component_faces,distance<0.)?;
        }
        // Reattach original wire edges and preserve metadata wherever an original
        // edge survives. This also covers the kernel-generated solid branch.
        let collapsed_vertex_survivors=collapsed_positions.as_ref().map(|positions|{
            let mut survivors=std::collections::HashMap::new();
            for (id,v) in mesh.vertices.iter().enumerate(){let p=positions.get(&id).copied().unwrap_or(v.position);survivors.entry(ingetrazo_core::math::PointKey::from_vec3(p)).or_insert(id);}
            survivors
        });
        let position=|id:usize|collapsed_positions.as_ref().and_then(|p|p.get(&id)).copied().unwrap_or(mesh.vertices[id].position);
        let untouched=|edge:&ingetrazo_core::mesh::MeshEdge|collapsed_vertex_survivors.as_ref().is_none_or(|survivors|survivors.get(&ingetrazo_core::math::PointKey::from_vec3(position(edge.v0)))==Some(&edge.v0)&&survivors.get(&ingetrazo_core::math::PointKey::from_vec3(position(edge.v1)))==Some(&edge.v1));
        // _weld_into reuses an edge between surviving vertices; recreated
        // edges default their flags. Surviving edges take precedence even
        // when face reconstruction changed the edge-vector order.
        let mut collapsed_edge_survivors=std::collections::HashSet::new();
        let ordered_edges = if collapsed_positions.is_some() {
            let mut surviving = Vec::with_capacity(mesh.edges.len());
            let mut recreated = Vec::new();
            for edge in &mesh.edges { let keep = untouched(edge); if keep { surviving.push((edge, keep)); } else { recreated.push((edge, keep)); } }
            surviving.extend(recreated); surviving
        } else { mesh.edges.iter().map(|edge| (edge, true)).collect() };
        let exact_position = |p:DVec3| [if p.x==0.0 {0} else {p.x.to_bits()},if p.y==0.0 {0} else {p.y.to_bits()},if p.z==0.0 {0} else {p.z.to_bits()}];
        let mut vertex_indices=std::collections::HashMap::new();
        for (id,v) in candidate.vertices.iter().enumerate(){vertex_indices.entry(exact_position(v.position)).or_insert(id);}
        for (edge, untouched) in ordered_edges {
            if collapsed_positions.is_none() && !edge.faces.is_empty()
                && edge.attrs.is_empty()
                && !edge.soft
                && edge.curve_id.is_none()
                && edge.layer.is_none()
                && !edge.hidden
            {continue;}
            let a=position(edge.v0);let b=position(edge.v1);
            if a==b {continue;}
            if collapsed_positions.is_some(){let mut ends=[ingetrazo_core::math::PointKey::from_vec3(a),ingetrazo_core::math::PointKey::from_vec3(b)];ends.sort_unstable();if !collapsed_edge_survivors.insert(ends){continue;}}
            let va = vertex_indices.get(&exact_position(a)).copied();
            let vb = vertex_indices.get(&exact_position(b)).copied();
            let id = if edge.faces.is_empty() {
                let va = candidate.add_vertex(a);
                let vb = candidate.add_vertex(b);
                vertex_indices.entry(exact_position(a)).or_insert(va);
                vertex_indices.entry(exact_position(b)).or_insert(vb);
                candidate.add_edge(va, vb)
            } else {
                va.zip(vb).and_then(|(a, b)| candidate.find_edge(a, b))
            };
            if let Some(id) = id {
                candidate.edges[id].attrs = edge.attrs.clone();
                if untouched {candidate.edges[id].history_id=edge.history_id;}
                candidate.edges[id].soft = untouched&&edge.soft;
                candidate.edges[id].curve_id = untouched.then_some(edge.curve_id).flatten();
                candidate.edges[id].layer = untouched.then(||edge.layer.clone()).flatten();
                candidate.edges[id].hidden = untouched&&edge.hidden;
            }
        }
        for vertex in &mesh.vertices {
            if vertex.edges.is_empty() {
                candidate.add_vertex(vertex.position);
            }
        }
        Self::collapse_redundant_sweep_vertices(&mut candidate, rings.iter().flatten().chain(shifted.iter().flatten()).copied());
        candidate.preserve_vertex_uids_from(mesh);
        if let Some(survivors)=&collapsed_vertex_survivors {for vertex in &mut candidate.vertices {if let Some(&id)=survivors.get(&ingetrazo_core::math::PointKey::from_vec3(vertex.position)){vertex.uid=mesh.vertices[id].uid;}}}
        // Unchanged authored hole loops retain their source winding. The
        // kernel's canonical hole winding is only for newly cut contours.
        if mesh.faces.iter().any(|f|!f.holes.is_empty()) && candidate.faces.iter().any(|f|!f.holes.is_empty()) {
            let mut original_holes=std::collections::HashMap::new();
            for ring in mesh.faces.iter().flat_map(|f|&f.holes) {
                let ordered=ring.iter().map(|&v|mesh.vertices[v].uid).collect::<Vec<_>>();
                let mut key=ordered.clone();key.sort_unstable();
                original_holes.entry(key).or_insert(ordered);
            }
            let candidate_indices=candidate.vertices.iter().enumerate().map(|(i,v)|(v.uid,i)).collect::<std::collections::HashMap<_,_>>();
            for f in &mut candidate.faces {
                for hole in &mut f.holes {
                    let mut key=hole.iter().map(|&v|candidate.vertices[v].uid).collect::<Vec<_>>();key.sort_unstable();
                    if let Some(original)=original_holes.get(&key) {
                        *hole=original.iter().map(|uid|candidate_indices[uid]).collect();
                    }
                }
            }
        }
        // Original stamps only faces carrying this push's genuinely new
        // vertices. Rebuilt neighbours over old vertices retain their paint.
        let old_vertices=mesh.vertices.iter().map(|v|v.uid).collect::<std::collections::HashSet<_>>();
        for f in &mut candidate.faces {
            let fresh=f.vertices.iter().chain(f.holes.iter().flatten()).any(|&v|!old_vertices.contains(&candidate.vertices[v].uid));
            if !fresh {continue;}
            if !["color","mat","texture","opacity"].iter().any(|key|f.attrs.contains_key(*key)) {
                for key in ["color","mat","texture","opacity"] {if let Some(value)=wall_source.attrs.get(key) {f.attrs.insert(key.into(),value.clone());}}
            }
            if !f.attrs.get("ifc").is_some_and(|v|match v {serde_json::Value::Null=>false,serde_json::Value::Bool(v)=>*v,serde_json::Value::Number(v)=>v.as_f64()!=Some(0.),serde_json::Value::String(v)=>!v.is_empty(),serde_json::Value::Array(v)=>!v.is_empty(),serde_json::Value::Object(v)=>!v.is_empty()}) {
                if let Some(tag)=wall_source.attrs.get("ifc") {f.attrs.insert("ifc".into(),tag.clone());}
            }
        }
        // No partial mutation or version bump escapes a failed preparation.
        if collapsed_positions.is_none(){ingetrazo_core::heal::refresh_interior(&mut candidate)?;}
        for id in 0..candidate.faces.len() {
            if !candidate.faces[id].interior { continue; }
            if ctrl_rebuilt_planes.iter().any(|&(point, normal)| {
                candidate.face_normal(id).dot(normal) < -0.999
                    && candidate.faces[id].vertices.iter().chain(candidate.faces[id].holes.iter().flatten())
                        .all(|&v| (candidate.vertices[v].position-point).dot(normal).abs()<1e-6)
            }) {
                candidate.faces[id].vertices.reverse();
                for hole in &mut candidate.faces[id].holes { hole.reverse(); }
            }
        }
        if keep_base&&distance<0. {
            // Source marks the new inward slab as interior before orienting;
            // its authored cap retains the original face winding.
            let cap_origin=origin+translation;
            let reverse=(0..candidate.faces.len()).filter(|&id|candidate.faces[id].interior&&candidate.face_normal(id).dot(normal)<0.&&candidate.faces[id].vertices.iter().all(|&v|(candidate.vertices[v].position-cap_origin).dot(normal).abs()<1e-6)).collect::<Vec<_>>();
            for id in reverse {candidate.faces[id].vertices.reverse();for hole in &mut candidate.faces[id].holes {hole.reverse();}}
        }
        if keep_base && was_solid {
            let seed = candidate
                .faces
                .iter()
                .position(|f| {
                    f.vertices
                        .iter()
                        .any(|&v| candidate.vertices[v].position.distance(origin) < 1e-6)
                        && f.vertices.iter().all(|&v| {
                            (candidate.vertices[v].position - origin).dot(normal).abs() < 1e-6
                        })
                })
                .ok_or("Ctrl copy lost its starting slab")?;
            let mut incident = std::collections::BTreeMap::<(usize, usize), Vec<usize>>::new();
            for (id, f) in candidate.faces.iter().enumerate() {
                for ring in std::iter::once(&f.vertices).chain(&f.holes) {
                    for i in 0..ring.len() {
                        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                        incident.entry((a.min(b), a.max(b))).or_default().push(id);
                    }
                }
            }
            let mut faces = std::collections::HashSet::from([seed]);
            let mut queue = vec![seed];
            while let Some(id) = queue.pop() {
                let f = &candidate.faces[id];
                for ring in std::iter::once(&f.vertices).chain(&f.holes) {
                    for i in 0..ring.len() {
                        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                        for &next in &incident[&(a.min(b), a.max(b))] {
                            if faces.insert(next) {
                                queue.push(next);
                            }
                        }
                    }
                }
            }
            let mut ids: Vec<_> = faces.into_iter().collect();
            ids.sort_unstable();
            let body = ingetrazo_core::pieces::copy_part(
                &candidate,
                &ingetrazo_core::pieces::Piece {
                    // Interior partitions remain document geometry, but do not
                    // belong to the solid boundary being admitted. In particular
                    // an edge joining two interior faces has zero boundary
                    // incidence and cannot invalidate an otherwise closed shell.
                    faces: ids.into_iter().filter(|&id| !candidate.faces[id].interior).collect(),
                    edges: vec![],
                },
            )?;
            ingetrazo_core::solids::validate_solid_mesh(&body)
                .map_err(|e|format!("Ctrl copy would leave an invalid solid: {e}. Use a normal Push/Pull or a smaller copy."))?;
        }
        scene.mesh = candidate;
        scene.clear_selection();
        scene.bump_version();
        Ok(())
    }
}

impl Tool for PushPullTool {
    fn tool_type(&self) -> ToolType {
        ToolType::PushPull
    }

    fn name(&self) -> &'static str {
        "Push / Pull"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("P")
    }

    fn description(&self) -> &'static str {
        "Push or pull a face to extrude it into a solid or cut it back."
    }

    fn vcb_label(&self) -> &'static str {
        "Distance"
    }

    fn on_activate(&mut self, _scene: &mut Scene) {
        self.reset();
    }

    fn on_deactivate(&mut self, _scene: &mut Scene) {
        self.reset();
    }

    fn take_history_acquisition_faces(&mut self)->Vec<(usize,ingetrazo_core::mesh::MeshFace)> {
        std::mem::take(&mut self.acquisition_history_faces)
    }
    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.reset();
        self.limit_status=None;
        self.acquisition_history_faces.clear();
    }

    fn face_sweep_preview(&self)->Option<crate::base::FaceSweepPreview>{
        let(face_identity,mesh_revision,attached,prism_cap,anchor)=self.sweep_identity?;
        Some(crate::base::FaceSweepPreview{face:self.selected_face?,face_identity,mesh_revision,anchor,normal:self.acquired_normal?,offset:(self.acquired_normal?.as_vec3()*(self.extrusion_distance as f32)).as_dvec3(),attached,prism_cap,keep_base:self.keep_base})
    }

    fn distance_label(&self)->Option<(f64,DVec3)> {
        if !self.is_dragging {return None;}
        let anchor=self.sweep_identity?.4;
        let midpoint=(anchor.as_vec3()+self.acquired_normal?.as_vec3()*((self.extrusion_distance*0.5)as f32)).as_dvec3();
        Some((self.extrusion_distance.abs(),midpoint))
    }

    fn status_message(&self)->Option<&str>{self.limit_status.as_deref()}
    fn take_status_message(&mut self)->Option<String>{self.limit_status.take()}
    fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }
    fn on_move(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.keep_base = ctx.ctrl_held;
        if !self.is_dragging {
            self.hovered_face = Self::eligible_face_at(scene,ctx).ok().flatten();
        } else if let (Some(face_id), Some(start)) = (self.selected_face, self.start_point) {
            let normal = self.acquired_normal.unwrap_or_else(||scene.mesh.face_normal(face_id));
            let raw=(ctx.world-start).dot(normal);
            self.extrusion_distance=if raw<0. {self.inward_limit.map_or(raw,|limit|raw.max(-limit))}else{raw};
            if self.extrusion_distance!=raw {self.limit_status=Some(format!("Push limited to {:.2} m — deeper would leave the solid",-self.extrusion_distance));}
        }
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.keep_base = ctx.ctrl_held;
        if !self.is_dragging {
            let face_pick = self.acquire_face(scene,ctx);

            if let Some(face_id) = face_pick {
                self.limit_status=None;
                if let Err(error)=self.prepare_acquisition(scene,face_id) {
                    self.error=Some(error);return;
                }
                self.selected_face = Some(face_id);
                let normal=Self::acquisition_normal(&scene.mesh,face_id);
                self.acquired_normal=Some(normal);
                let f=&scene.mesh.faces[face_id];
                let attached=std::iter::once(&f.vertices).chain(&f.holes).all(|ring|(0..ring.len()).all(|i|scene.mesh.find_edge(ring[i],ring[(i+1)%ring.len()]).is_some_and(|e|scene.mesh.edges[e].faces.iter().any(|&other|other!=face_id))));
                let prism=attached && std::iter::once(&f.vertices).chain(&f.holes).all(|ring|(0..ring.len()).all(|i|scene.mesh.find_edge(ring[i],ring[(i+1)%ring.len()]).is_some_and(|e|scene.mesh.edges[e].faces.iter().filter(|&&other|other!=face_id).all(|&other|scene.mesh.face_normal(other).dot(normal).abs()<0.001))));
                let anchor=(f.vertices.iter().map(|&v|scene.mesh.vertices[v].position.as_vec3().as_dvec3()).sum::<DVec3>()/f.vertices.len()as f64).as_vec3().as_dvec3();
                self.sweep_identity=Some((f.history_id,scene.version,attached,prism,anchor));

                self.inward_limit=inward_limit::compute_with_normal(&scene.mesh,face_id,ctx.world,Some(normal));
                self.start_point = Some(ctx.world);
                self.is_dragging = true;
                self.extrusion_distance = 0.0;
            }
        } else if let Some(face_id) = self.selected_face {
            let dist = self.extrusion_distance;
            if dist.abs()<Self::MIN_EXTRUDE{return;}
            match self.commit_distance(scene, face_id, dist, self.keep_base) {
                Ok(actual) => {
                    self.last_distance = Some(actual);
                    self.reset_after_commit();
                }
                Err(error) => self.error = Some(error),
            }
        }
    }

    fn on_double_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        let face_pick = self.acquire_face(scene,ctx);
        if face_pick.is_none(){return;}
        if let Some(last_d) = self.last_distance {

            if let Some(face_id) = face_pick {
                if let Err(error)=self.prepare_acquisition(scene,face_id) {
                    self.error=Some(error);return;
                }
                self.acquired_normal=Some(Self::acquisition_normal(&scene.mesh,face_id));
                self.inward_limit=None;
                match self.commit_distance(scene, face_id, last_d, ctx.ctrl_held) {
                    Ok(actual) => {self.last_distance=Some(actual);self.reset_after_commit();},
                    Err(error) => self.error = Some(error),
                }
                return;
            }
        }
        self.on_click(scene, ctx);
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let value=ingetrazo_core::units::vcb::parse_single(text,ingetrazo_core::units::model_scale(scene))?;
        let requested=if self.extrusion_distance<0. {-value}else{value};
        if requested.abs()<Self::MIN_EXTRUDE {return Err("Push/Pull distance is below the minimum extrusion".into());}
        let dist=if requested<0.&&self.selected_face.is_some(){self.inward_limit.map_or(requested,|limit|requested.max(-limit))}else{self.selected_face.or(self.hovered_face).map_or(requested,|id|Self::limited_distance(&scene.mesh,id,requested,self.keep_base))};

        if dist!=requested {self.limit_status=Some(format!("Push limited to {:.2} m — deeper would leave the solid",-dist));}
        if let Some(face_id)=self.selected_face.or(self.hovered_face){
            match self.commit_distance(scene,face_id,dist,self.keep_base){
                Ok(actual)=>{self.last_distance=Some(actual);self.reset_after_commit();Ok(())},
                Err(error)=>{self.error=Some(error.clone());Err(error)}
            }
        } else {
            Err("No face selected for Push/Pull".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    fn make_test_square_scene() -> Scene {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = scene.mesh.add_vertex(DVec3::new(1.0, 0.0, 0.0));
        let v2 = scene.mesh.add_vertex(DVec3::new(1.0, 1.0, 0.0));
        let v3 = scene.mesh.add_vertex(DVec3::new(0.0, 1.0, 0.0));
        scene.mesh.add_face(vec![v0, v1, v2, v3], vec![]);
        scene
    }

    #[test]
    fn triangle_boundary_admission_rejects_actual_corruptions() {
        let mut scene=make_test_square_scene();
        PushPullTool::extrude_face_with_options(&mut scene,0,1.,false).unwrap();
        assert!(PushPullTool::validate_triangle_boundary(&scene.mesh).is_ok());
        let mut missing=scene.mesh.clone();missing.faces.pop();
        assert!(PushPullTool::validate_triangle_boundary(&missing).is_err());
        let mut reversed=scene.mesh.clone();reversed.faces[0].vertices.reverse();
        assert!(PushPullTool::validate_triangle_boundary(&reversed).is_err());
        let mut doubled=scene.mesh.clone();doubled.faces.push(doubled.faces[0].clone());
        assert!(PushPullTool::validate_triangle_boundary(&doubled).is_err());
        let mut nonfinite=scene.mesh.clone();nonfinite.vertices[0].position.x=f64::NAN;
        assert!(PushPullTool::validate_triangle_boundary(&nonfinite).is_err());
    }

    #[test]
    fn stale_hover_cannot_start_after_layer_lock() {
        let mut scene=Scene::new();
        let vertices=[DVec3::ZERO,DVec3::X,DVec3::X+DVec3::Y,DVec3::Y].map(|p|scene.mesh.add_vertex(p));
        let id=scene.mesh.add_face(vertices.to_vec(),vec![]).unwrap();
        scene.mesh.faces[id].layer=Some("Walls".into());
        scene.layers.push(ingetrazo_core::layers::Layer::new("Walls"));
        let ctx=ToolContext::new(DVec3::new(0.5,0.5,0.),ingetrazo_core::math::Ray::new(DVec3::new(0.5,0.5,2.),-DVec3::Z));
        let mut tool=PushPullTool::new();tool.on_move(&mut scene,&ctx);assert_eq!(tool.hovered_face,Some(id));
        scene.layers.last_mut().unwrap().locked=true;let before=scene.clone();tool.on_click(&mut scene,&ctx);
        assert!(!tool.is_dragging);assert_eq!(tool.hovered_face,None);assert_eq!(scene,before);
    }

    #[test]
    fn repeated_ctrl_invalid_source_classification_is_rejected_atomically() {
        // Original accepts two slabs but its orient pass loses both interior
        // flags (parity cancellation), leaving a nonsolid 16-face result.
        // Retain the known source failure as an atomic refusal in Rust.
        let mut scene = make_test_square_scene();
        scene.mesh.faces[0]
            .attrs
            .insert("mat".into(), serde_json::json!("Stone"));
        PushPullTool::extrude_face(&mut scene, 0, 3.).unwrap();
        let top = scene
            .mesh
            .faces
            .iter()
            .position(|f| {
                f.vertices
                    .iter()
                    .all(|&v| (scene.mesh.vertices[v].position.z - 3.).abs() < 1e-8)
            })
            .unwrap();
        PushPullTool::extrude_face_with_options(&mut scene, top, 2., true).unwrap();
        let before = scene.clone();
        let top = scene
            .mesh
            .faces
            .iter()
            .position(|f| {
                f.vertices
                    .iter()
                    .all(|&v| (scene.mesh.vertices[v].position.z - 5.).abs() < 1e-8)
            })
            .unwrap();
        let error = PushPullTool::extrude_face_with_options(&mut scene, top, 2., true).unwrap_err();
        assert!(error.contains("Ctrl copy would leave an invalid solid"));
        assert_eq!(scene, before);
    }

    #[test]
    fn ctrl_copy_validates_its_body_and_retains_unrelated_open_geometry() {
        let mut scene = make_test_square_scene();
        PushPullTool::extrude_face(&mut scene, 0, 3.).unwrap();
        let top = scene
            .mesh
            .faces
            .iter()
            .position(|f| {
                f.vertices
                    .iter()
                    .all(|&v| (scene.mesh.vertices[v].position.z - 3.).abs() < 1e-8)
            })
            .unwrap();
        let ids = [
            DVec3::new(10., 0., 0.),
            DVec3::new(11., 0., 0.),
            DVec3::new(11., 1., 0.),
            DVec3::new(10., 1., 0.),
        ]
        .map(|p| scene.mesh.add_vertex(p));
        let sheet = scene.mesh.add_face(ids.to_vec(), vec![]).unwrap();
        scene.mesh.faces[sheet]
            .attrs
            .insert("sheet".into(), serde_json::json!("retained"));
        let a = scene.mesh.add_vertex(DVec3::new(20., 0., 0.));
        let b = scene.mesh.add_vertex(DVec3::new(21., 0., 0.));
        let wire = scene.mesh.add_edge(a, b).unwrap();
        scene.mesh.edges[wire]
            .attrs
            .insert("wire".into(), serde_json::json!("retained"));
        PushPullTool::extrude_face_with_options(&mut scene, top, 2., true).unwrap();
        assert_eq!(scene.mesh.faces.len(), 12);
        assert!(scene
            .mesh
            .faces
            .iter()
            .any(|f| f.attrs.get("sheet") == Some(&serde_json::json!("retained"))));
        assert!(scene
            .mesh
            .edges
            .iter()
            .any(|e| e.attrs.get("wire") == Some(&serde_json::json!("retained"))));
    }

    #[test]
    fn ctrl_cap_copy_retains_slab_belts_and_inward_material() {
        for distance in [2., -2.] {
            let mut scene = make_test_square_scene();
            PushPullTool::extrude_face(&mut scene, 0, 3.).unwrap();
            let top = scene
                .mesh
                .faces
                .iter()
                .enumerate()
                .find(|(_, f)| {
                    f.vertices
                        .iter()
                        .all(|&v| (scene.mesh.vertices[v].position.z - 3.).abs() < 1e-8)
                })
                .map(|(i, _)| i)
                .unwrap();
            let ids: Vec<_> = scene.mesh.vertices.iter().map(|v| v.uid).collect();
            PushPullTool::extrude_face_with_options(&mut scene, top, distance, true).unwrap();
            assert_eq!(scene.mesh.faces.len(), 11);
            assert_eq!(scene.mesh.faces.iter().filter(|f| f.interior).count(), 1);
            assert!(
                (scene.mesh.volume().unwrap() - if distance > 0. { 5. } else { 3. }).abs() < 1e-8
            );
            assert!(ids
                .iter()
                .all(|id| scene.mesh.vertices.iter().any(|v| v.uid == *id)));
            let belt = if distance > 0. { 3. } else { 1. };
            let edges: Vec<_> = scene
                .mesh
                .edges
                .iter()
                .filter(|e| {
                    (scene.mesh.vertices[e.v0].position.z - belt).abs() < 1e-8
                        && (scene.mesh.vertices[e.v1].position.z - belt).abs() < 1e-8
                })
                .collect();
            assert_eq!(edges.len(), 4);
            assert!(edges.iter().all(|e| e.faces.len() == 3));
            ingetrazo_core::solids::validate_solid_mesh(&scene.mesh).unwrap();
        }
    }

    #[test]
    fn collapse_preserves_protected_base_subdivision() {
        let mut scene=make_test_square_scene();
        PushPullTool::extrude_face(&mut scene,0,1.).unwrap();
        let base=scene.mesh.faces.iter().position(|f|f.vertices.iter().all(|&v|scene.mesh.vertices[v].position.z==0.)).unwrap();
        let ids=scene.mesh.faces[base].vertices.clone();
        let old=scene.mesh.clone();scene.mesh=ingetrazo_core::mesh::Mesh::new();
        for v in &old.vertices {scene.mesh.add_vertex(v.position);}
        for (i,f) in old.faces.iter().enumerate() {if i!=base {scene.mesh.add_face(f.vertices.clone(),f.holes.clone()).unwrap();}}
        for ring in [vec![ids[0],ids[1],ids[2]],vec![ids[0],ids[2],ids[3]]] {
            let id=scene.mesh.add_face(ring,vec![]).unwrap();scene.mesh.faces[id].attrs.insert("vendor".into(),serde_json::json!(71));
        }
        let cap=scene.mesh.faces.iter().position(|f|f.vertices.iter().all(|&v|scene.mesh.vertices[v].position.z==1.)).unwrap();
        PushPullTool::extrude_face(&mut scene,cap,-1.).unwrap();
        assert_eq!(scene.mesh.faces.len(),3);assert_eq!(scene.mesh.vertices.len(),4);assert_eq!(scene.mesh.edges.len(),5);
        assert_eq!(scene.mesh.faces.iter().filter(|f|f.attrs.get("vendor")==Some(&serde_json::json!(71))).count(),2);
        assert_eq!(scene.mesh.faces.iter().filter(|f|f.holes.len()==2).count(),1);
        assert!(scene.mesh.faces.iter().all(|f|!f.interior));
    }

    #[test]
    fn test_pushpull_extrude_cube() {
        let mut scene = make_test_square_scene();
        let mut tool = PushPullTool::new();

        // Hover & click face 0
        let ray = Ray::new(DVec3::new(0.5, 0.5, 5.0), -DVec3::Z);
        let ctx0 = ToolContext::new(DVec3::new(0.5, 0.5, 0.0), ray);
        tool.on_move(&mut scene, &ctx0);
        assert_eq!(tool.hovered_face, Some(0));

        tool.on_click(&mut scene, &ctx0);
        assert!(tool.is_dragging);

        // Move up by 1.0
        let ctx1 = ToolContext::new(DVec3::new(0.5, 0.5, 1.0), ray);
        tool.on_move(&mut scene, &ctx1);
        assert!((tool.extrusion_distance - 1.0).abs() < 1e-4);

        // Commit click
        tool.on_click(&mut scene, &ctx1);
        assert!(!tool.is_dragging);
        assert_eq!(tool.last_distance, Some(1.0));

        // Result is a 6-sided solid cube!
        assert_eq!(scene.mesh.faces.len(), 6);
        assert!(scene.mesh.is_manifold_solid());
        let vol = scene.mesh.volume().unwrap();
        assert!((vol - 1.0).abs() < 1e-3);
    }

    #[test]
    fn test_pushpull_vcb_and_repeat() {
        let mut scene = make_test_square_scene();
        let mut tool = PushPullTool::new();

        let ray = Ray::new(DVec3::new(0.5, 0.5, 5.0), -DVec3::Z);
        let ctx0 = ToolContext::new(DVec3::new(0.5, 0.5, 0.0), ray);
        tool.on_click(&mut scene, &ctx0);

        // Extrude 2.5 via VCB
        assert!(tool.on_vcb_input(&mut scene, "2.5").is_ok());
        assert_eq!(tool.last_distance, Some(2.5));
        assert!(scene.mesh.is_manifold_solid());
        let vol = scene.mesh.volume().unwrap();
        assert!((vol - 2.5).abs() < 1e-3);
    }
    #[test]
    fn repeated_cap_pushes_keep_whole_polygon_and_metadata() {
        let mut scene = make_test_square_scene();
        scene.mesh.faces[0]
            .attrs
            .insert("mat".into(), "stone".into());
        PushPullTool::extrude_face(&mut scene, 0, 2.0).unwrap();
        for (distance, expected) in [(1.0, 3.0), (-0.5, 2.5)] {
            let top = (0..scene.mesh.faces.len())
                .max_by(|&a, &b| {
                    scene
                        .mesh
                        .face_centroid(a)
                        .z
                        .total_cmp(&scene.mesh.face_centroid(b).z)
                })
                .unwrap();
            assert_eq!(scene.mesh.faces[top].vertices.len(), 4);
            PushPullTool::extrude_face(&mut scene, top, distance).unwrap();
            assert!((scene.mesh.volume().unwrap() - expected).abs() < 1e-8);
            assert!(scene
                .mesh
                .faces
                .iter()
                .all(|f| f.attrs.get("mat") == Some(&"stone".into())));
        }
    }

    #[test]
    fn invalid_or_sub_weld_extrusion_is_atomic() {
        let mut scene = make_test_square_scene();
        for distance in [f64::NAN, f64::INFINITY] {
            let before = scene.clone();
            assert!(PushPullTool::extrude_face(&mut scene, 0, distance).is_err());
            assert_eq!(scene, before);
        }
        for distance in [0.,0.00002,-0.00002] {
            let before=scene.clone();PushPullTool::extrude_face(&mut scene,0,distance).unwrap();assert_eq!(scene,before);
        }
        scene.mesh.vertices[0].position.z = 0.01;
        let before = scene.clone();
        assert!(PushPullTool::extrude_face(&mut scene, 0, 1.0).is_err());
        assert_eq!(scene, before);
    }
    #[test]
    fn source_wall_paint_drops_vendor_and_plane_map_but_caps_keep_them(){
        let mut scene=make_test_square_scene();
        scene.mesh.faces[0].layer=Some("Walls".into());
        scene.mesh.faces[0].attrs=serde_json::from_value(serde_json::json!({"vendor":71,"color":[1,0,0],"texture":{"path":"wall.png","uvw":[1,0,0,0,0,1,0,0],"tile":2},"ifc":{"id":4,"class":"IfcSlab"}})).unwrap();
        PushPullTool::extrude_face(&mut scene,0,1.).unwrap();
        let caps=scene.mesh.faces.iter().filter(|f|f.attrs.contains_key("vendor")).count();assert_eq!(caps,2);
        for id in 0..scene.mesh.faces.len(){let f=&scene.mesh.faces[id];if scene.mesh.face_normal(id).z.abs()<0.1{assert!(!f.attrs.contains_key("vendor"));assert!(f.layer.is_none());assert!(f.attrs["texture"].get("uvw").is_none());assert_eq!(f.attrs["texture"]["planar"],true);assert_eq!(f.attrs["ifc"]["id"],4);}}
    }
    #[test]
    fn scoped_authoring_rounding_retains_unrelated_f64_vertices_and_atomic_tag_refusal(){
        let mut scene=make_test_square_scene();let point=DVec3::new(100.,0.,1.+2_f64.powi(-40));scene.mesh.add_vertex(point);PushPullTool::extrude_face(&mut scene,0,0.6).unwrap();assert!(scene.mesh.vertices.iter().any(|v|v.position==point));
        let mut scene=make_test_square_scene();scene.mesh.faces[0].attrs.insert("ifc".into(),serde_json::json!("opaque imported tag"));let before=scene.clone();assert!(PushPullTool::extrude_face(&mut scene,0,1.).is_err());assert_eq!(scene,before);
    }

    #[test]
    fn source_collinear_cleanup_preserves_junctions_unrelated_records_and_face_identity(){
        use ingetrazo_core::mesh::Mesh;
        let mut m=Mesh::new();
        let pts=[DVec3::ZERO,DVec3::X,DVec3::X*2.,DVec3::new(2.,1.,0.),DVec3::Y];
        let ring=pts.into_iter().map(|p|m.add_vertex(p)).collect();let f=m.add_face(ring,vec![]).unwrap();
        m.faces[f].attrs.insert("vendor".into(),serde_json::json!(17));let identity=m.faces[f].history_id;
        let unrelated=m.add_vertex(DVec3::new(100.,0.,1.+2_f64.powi(-40)));let uid=m.vertices[unrelated].uid;
        // A disconnected subdivided face lies inside the sweep's bounds.
        let detached=[DVec3::new(0.2,0.2,0.5),DVec3::new(0.5,0.2,0.5),DVec3::new(0.8,0.2,0.5),DVec3::new(0.8,0.8,0.5),DVec3::new(0.2,0.8,0.5)].into_iter().map(|p|m.add_vertex(p)).collect();
        let detached=m.add_face(detached,vec![]).unwrap();let retained=m.faces[detached].clone();let retained_vertices=retained.vertices.iter().map(|&v|(m.vertices[v].uid,m.vertices[v].position)).collect::<Vec<_>>();
        let mut junction=m.clone();let station=1;let branch=junction.add_vertex(DVec3::new(1.,-1.,0.));let edge=junction.add_edge(station,branch).unwrap();junction.edges[edge].attrs.insert("wire".into(),serde_json::json!(true));let wire=junction.edges[edge].history_id;
        PushPullTool::collapse_redundant_sweep_vertices(&mut junction,pts.into_iter());
        assert_eq!(junction.faces[0].vertices.len(),5);assert!(junction.edges.iter().any(|e|e.history_id==wire&&e.attrs.contains_key("wire")));
        PushPullTool::collapse_redundant_sweep_vertices(&mut m,pts.into_iter().chain([DVec3::new(2.,1.,1.)]));
        assert_eq!(m.faces[0].vertices.len(),4);assert_eq!(m.faces[0].history_id,identity);assert_eq!(m.faces[0].attrs["vendor"],17);
        assert!(m.vertices.iter().any(|v|v.uid==uid&&v.position.z==1.+2_f64.powi(-40)));assert_eq!(m.edges.len(),9);assert_eq!(m.faces[1].history_id,retained.history_id);assert_eq!(m.faces[1].vertices.len(),5);assert_eq!(m.faces[1].vertices.iter().map(|&v|(m.vertices[v].uid,m.vertices[v].position)).collect::<Vec<_>>(),retained_vertices);
    }

}
