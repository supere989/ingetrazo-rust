// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use glam::DVec3;
use ingetrazo_core::scene::Scene;

use crate::base::{Tool, ToolContext, ToolType};

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
}

impl PushPullTool {
    pub fn new() -> Self {
        Self::default()
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

    const MIN_EXTRUDE:f64=2e-4;
    // Clean prism caps have a parallel supporting cap. Its boundary contact
    // is a blocker, unlike a strictly contained through-hole mouth.
    fn cap_limit(mesh:&ingetrazo_core::mesh::Mesh,face_id:usize)->Option<f64> {
        let face=mesh.faces.get(face_id)?;let normal=mesh.face_normal(face_id);
        let clean=std::iter::once(&face.vertices).chain(&face.holes).all(|ring|(0..ring.len()).all(|i| {
            let edge=mesh.find_edge(ring[i],ring[(i+1)%ring.len()]).and_then(|id|mesh.edges.get(id));
            edge.is_some_and(|e|e.faces.len()==2&&e.faces.iter().copied().filter(|&id|id!=face_id).all(|id| {
                let other=&mesh.faces[id];mesh.face_normal(id).dot(normal).abs()<0.001&&(0..other.vertices.len()).any(|i| {
                    let d=mesh.vertices[other.vertices[(i+1)%other.vertices.len()]].position-mesh.vertices[other.vertices[i]].position;
                    d.length()>1e-9&&d.normalize().dot(normal).abs()>0.999
                })
            }))
        }));
        if !clean{return None;}
        let origin=mesh.vertices[face.vertices[0]].position;
        (0..mesh.faces.len()).filter(|&id|id!=face_id&&mesh.face_normal(id).dot(normal).abs()>0.999).filter_map(|id| {
            let distance=(origin-mesh.vertices[mesh.faces[id].vertices[0]].position).dot(normal);
            if distance<=1e-6{return None;}
            let triangles=mesh.triangulate_face(id);
            face.vertices.iter().all(|&v| {
                let ray=ingetrazo_core::math::Ray::new(mesh.vertices[v].position,-normal);
                triangles.iter().any(|&(a,b,c)|ingetrazo_core::math::ray_triangle_intersect(&ray,a,b,c).is_some_and(|(t,_)|(t-distance).abs()<1e-6))
            }).then_some(distance)
        }).min_by(f64::total_cmp)
    }
    fn limited_distance(mesh:&ingetrazo_core::mesh::Mesh,face:usize,d:f64,keep:bool)->f64 {
        if !keep&&d<0. {Self::cap_limit(mesh,face).map_or(d,|limit|d.max(-limit))}else{d}
    }

    pub fn reset(&mut self) {
        self.hovered_face = None;
        self.selected_face = None;
        self.start_point = None;
        self.inward_limit=None;
        self.extrusion_distance = 0.0;
        self.is_dragging = false;
        self.keep_base = false;
        self.error = None;
    }

    /// Prepare the whole extrusion before changing the scene. A free face
    /// becomes a closed prism; a bordered surface loses its old base. Closed
    /// solids use union/difference so inward pushes remove material and outward
    /// pushes remove the former cap/internal partitions.
    pub fn extrude_face(scene: &mut Scene, face_id: usize, distance: f64) -> Result<(), String> {
        Self::extrude_face_with_options(scene, face_id, distance, false)
    }
    /// Ctrl copies a segment and retains the original cap as a slab division.
    pub fn extrude_face_with_options(
        scene: &mut Scene,
        face_id: usize,
        distance: f64,
        keep_base: bool,
    ) -> Result<(), String> {
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
        let distance=Self::limited_distance(mesh,face_id,distance,keep_base);
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
        let translation=(normal.as_vec3()*(distance as f32)).as_dvec3();
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
        let was_solid = solid_report(&component).0;
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
        let mut candidate = if prism_cap && was_solid && !keep_base {
            let mut result=mesh.clone();
            let moving=face.vertices.iter().chain(face.holes.iter().flatten()).copied().collect::<std::collections::HashSet<_>>();
            for &id in &moving {result.vertices[id].position=(result.vertices[id].position.as_vec3()+translation.as_vec3()).as_dvec3();}
            let end=origin+translation;
            let flat=component_faces.iter().all(|&id|result.faces[id].vertices.iter().chain(result.faces[id].holes.iter().flatten()).all(|&v|(result.vertices[v].position-end).dot(normal).abs()<1e-6));
            if flat {
                // Full collapse is a valid surface endpoint, not an empty
                // Boolean result. Preserve the authored cap and its holes/paint.
                let mut collapsed=Mesh::new();
                let mut flat_rings=shifted.clone();
                let stationary_faces=component_faces.iter().copied().filter(|&id|!mesh.faces[id].vertices.iter().any(|v|moving.contains(v))&&mesh.face_normal(id).dot(normal).abs()>0.999).collect::<Vec<_>>();
                if stationary_faces.len()!=1 {return Err("Full collapse with subdivided opposing faces is not yet supported; geometry and metadata left unchanged".into());}
                let stationary=stationary_faces.first().copied();
                if stationary.is_some_and(|id|mesh.face_normal(id).dot(normal)<0.) {for ring in &mut flat_rings {ring.reverse();}}
                add_polygon(&mut collapsed,&flat_rings[0],&flat_rings[1..],face)?;
                for (id,other) in mesh.faces.iter().enumerate() {
                    if component_faces.contains(&id){continue;}
                    let points=other.vertices.iter().map(|&v|mesh.vertices[v].position).collect::<Vec<_>>();
                    let holes=other.holes.iter().map(|ring|ring.iter().map(|&v|mesh.vertices[v].position).collect()).collect::<Vec<Vec<_>>>();
                    add_polygon(&mut collapsed,&points,&holes,other)?;
                    collapsed.faces.last_mut().unwrap().interior=other.interior;
                }
                collapsed
            }else{
            if component_faces.iter().any(|&id|result.face_area(id)<1e-12) {
                // Landing a raised cap flush consumes its side walls. Repair
                // the closed boundary with the actual swept subtraction,
                // rather than rejecting a legitimate endpoint or retaining fins.
                let mut repaired=perform_solid_op(SolidOp::Subtract,&component,&prism)?;
                if repaired.faces.is_empty(){return Err("Endpoint has no valid surface boundary".into());}
                // Source endpoint rebuild gives the translated cap ownership
                // of the restored coplanar surface. Fuse only that swept plane.
                let plane_ids=(0..repaired.faces.len()).filter(|&id|repaired.face_normal(id).dot(normal)>0.999&&repaired.faces[id].vertices.iter().all(|&v|(repaired.vertices[v].position-end).dot(normal).abs()<1e-6)).collect::<Vec<_>>();
                if plane_ids.len()>1 {
                    let mut plane=ingetrazo_core::pieces::copy_part(&repaired,&ingetrazo_core::pieces::Piece{faces:plane_ids.clone(),edges:vec![]})?;
                    for f in &mut plane.faces {f.attrs=face.attrs.clone();f.layer=face.layer.clone();}
                    let mut triangles=Mesh::new();
                    for id in 0..plane.faces.len() {
                        for (a,b,c) in plane.triangulate_face(id) {add_polygon(&mut triangles,&[a,b,c],&[],&plane.faces[id])?;}
                    }
                    plane=triangles;
                    let (plane,_)=ingetrazo_core::simplify::simplify_mesh(&plane,0.,false)?;
                    let mut joined=Mesh::new();
                    for (source,ids) in [(&repaired,(0..repaired.faces.len()).filter(|id|!plane_ids.contains(id)).collect::<Vec<_>>()),(&plane,(0..plane.faces.len()).collect())] {
                        for id in ids {
                            let f=&source.faces[id];let points=f.vertices.iter().map(|&v|source.vertices[v].position).collect::<Vec<_>>();let holes=f.holes.iter().map(|r|r.iter().map(|&v|source.vertices[v].position).collect()).collect::<Vec<Vec<_>>>();add_polygon(&mut joined,&points,&holes,f)?;
                        }
                    }
                    repaired=ingetrazo_core::planar::stitch_boundaries(&joined)?;
                    ingetrazo_core::solids::validate_solid_mesh(&repaired)?;
                }
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
                ingetrazo_core::solids::validate_solid_mesh(&body).map_err(|e|format!("Push/Pull cap translation would leave an invalid solid: {e}"))?;
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
            }
            result
        };
        if was_solid && !keep_base && !prism_cap {
            let mut rims=vec![];
            for (base,cap) in rings.iter().zip(&shifted){for i in 0..base.len(){let j=(i+1)%base.len();rims.push((base[i],base[j]));rims.push((cap[i],cap[j]));rims.push((base[i],cap[i]));}}
            candidate=ingetrazo_core::solids::dissolve_rim_seams(&candidate,&rims)?;
        }
        // Reattach original wire edges and preserve metadata wherever an original
        // edge survives. This also covers the kernel-generated solid branch.
        for edge in &mesh.edges {
            if !edge.faces.is_empty()
                && edge.attrs.is_empty()
                && !edge.soft
                && edge.curve_id.is_none()
                && edge.layer.is_none()
                && !edge.hidden
            {
                continue;
            }
            let a = mesh.vertices[edge.v0].position;
            let b = mesh.vertices[edge.v1].position;
            let va = candidate.vertices.iter().position(|v| v.position == a);
            let vb = candidate.vertices.iter().position(|v| v.position == b);
            let id = if edge.faces.is_empty() {
                let va = candidate.add_vertex(a);
                let vb = candidate.add_vertex(b);
                candidate.add_edge(va, vb)
            } else {
                va.zip(vb).and_then(|(a, b)| candidate.find_edge(a, b))
            };
            if let Some(id) = id {
                candidate.edges[id].attrs = edge.attrs.clone();
                candidate.edges[id].soft = edge.soft;
                candidate.edges[id].curve_id = edge.curve_id;
                candidate.edges[id].layer = edge.layer.clone();
                candidate.edges[id].hidden = edge.hidden;
            }
        }
        for vertex in &mesh.vertices {
            if vertex.edges.is_empty() {
                candidate.add_vertex(vertex.position);
            }
        }
        candidate.preserve_vertex_uids_from(mesh);
        // Unchanged authored hole loops retain their source winding. The
        // kernel's canonical hole winding is only for newly cut contours.
        for f in &mut candidate.faces {
            for hole in &mut f.holes {
                let identities=hole.iter().map(|&v|candidate.vertices[v].uid).collect::<std::collections::HashSet<_>>();
                if let Some(original)=mesh.faces.iter().flat_map(|f|&f.holes).find(|ring|ring.len()==hole.len()&&ring.iter().all(|&v|identities.contains(&mesh.vertices[v].uid))) {
                    *hole=original.iter().map(|&v|candidate.vertices.iter().position(|point|point.uid==mesh.vertices[v].uid).unwrap()).collect();
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
        ingetrazo_core::heal::refresh_interior(&mut candidate)?;
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

    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.reset();
    }

    fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }
    fn on_move(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.keep_base = ctx.ctrl_held;
        if !self.is_dragging {
            self.hovered_face = Self::eligible_face_at(scene,ctx).ok().flatten();
        } else if let (Some(face_id), Some(start)) = (self.selected_face, self.start_point) {
            let normal = scene.mesh.face_normal(face_id);
            let raw=(ctx.world-start).dot(normal);
            self.extrusion_distance=if !self.keep_base&&raw<0. {self.inward_limit.map_or(raw,|limit|raw.max(-limit))}else{raw};
        }
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.keep_base = ctx.ctrl_held;
        if !self.is_dragging {
            let face_pick = self.acquire_face(scene,ctx);

            if let Some(face_id) = face_pick {
                self.selected_face = Some(face_id);
                self.inward_limit=Self::cap_limit(&scene.mesh,face_id);
                self.start_point = Some(ctx.world);
                self.is_dragging = true;
                self.extrusion_distance = 0.0;
            }
        } else if let Some(face_id) = self.selected_face {
            let dist = self.extrusion_distance;
            if dist.abs()<Self::MIN_EXTRUDE{return;}
            match Self::extrude_face_with_options(scene, face_id, dist, self.keep_base) {
                Ok(()) => {
                    self.last_distance = Some(dist);
                    self.reset();
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
                match Self::extrude_face_with_options(scene, face_id, last_d, ctx.ctrl_held) {
                    Ok(()) => self.reset(),
                    Err(error) => self.error = Some(error),
                }
                return;
            }
        }
        self.on_click(scene, ctx);
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let requested=ingetrazo_core::units::parse_length(text,ingetrazo_core::units::LengthUnit::Meters)?;
        if requested.abs()<Self::MIN_EXTRUDE {return Err("Push/Pull distance is below the minimum extrusion".into());}
        let dist=self.selected_face.or(self.hovered_face).map_or(requested,|id|Self::limited_distance(&scene.mesh,id,requested,self.keep_base));

        if let Some(face_id) = self.selected_face {
            if let Err(error) =
                Self::extrude_face_with_options(scene, face_id, dist, self.keep_base)
            {
                self.error = Some(error.clone());
                return Err(error);
            }
            self.last_distance = Some(dist);
            self.reset();
            Ok(())
        } else if let Some(face_id) = self.hovered_face {
            if let Err(error) =
                Self::extrude_face_with_options(scene, face_id, dist, self.keep_base)
            {
                self.error = Some(error.clone());
                return Err(error);
            }
            self.last_distance = Some(dist);
            self.reset();
            Ok(())
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
    fn collapse_with_protected_base_subdivision_is_atomic() {
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
        let before=scene.clone();
        let error=PushPullTool::extrude_face(&mut scene,cap,-1.).unwrap_err();
        assert!(error.contains("subdivided opposing"));assert_eq!(scene,before);
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

}
