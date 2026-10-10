// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Wavefront OBJ format export and import.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use glam::DVec3;

use ingetrazo_core::scene::Scene;

#[derive(thiserror::Error, Debug)]
pub enum ObjError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error at line {0}: {1}")]
    Parse(usize, String),
}

/// Export scene (loose mesh + groups) into an OBJ writer.
pub fn export_obj(scene: &Scene, writer: &mut impl Write) -> Result<(), ObjError> {
    writeln!(writer, "# IngeTrazo OBJ Export")?;

    let triangles=crate::exchange::triangles(scene);
    for triangle in &triangles {
        for p in triangle.points {writeln!(writer,"v {:.6} {:.6} {:.6}",p.x,p.y,p.z)?;}
    }
    for triangle in &triangles {
        let [a,b,c]=triangle.points;
        let n=(b-a).cross(c-a).normalize_or_zero();
        writeln!(writer,"vn {:.6} {:.6} {:.6}",n.x,n.y,n.z)?;
    }
    for (i,_) in triangles.iter().enumerate() {
        let v=3*i+1;let n=i+1;
        writeln!(writer,"f {v}//{n} {}//{n} {}//{n}",v+1,v+2)?;
    }

    Ok(())
}

/// Import an OBJ stream into a scene's mesh.
pub fn import_obj(reader: &mut impl Read, scene: &mut Scene) -> Result<(), ObjError> {
    let buf_reader = BufReader::new(reader);
    let mut file_verts: Vec<DVec3> = Vec::new();

    for (line_num, line_res) in buf_reader.lines().enumerate() {
        let line = line_res?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let mut parts = trimmed.split_whitespace();
        let cmd = match parts.next() {
            Some(c) => c,
            None => continue,
        };

        match cmd {
            "v" => {
                let x: f64 = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| {
                    ObjError::Parse(line_num + 1, "Invalid vertex X".to_string())
                })?;
                let y: f64 = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| {
                    ObjError::Parse(line_num + 1, "Invalid vertex Y".to_string())
                })?;
                let z: f64 = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| {
                    ObjError::Parse(line_num + 1, "Invalid vertex Z".to_string())
                })?;
                file_verts.push(DVec3::new(x, y, z));
            }
            "f" => {
                let mut face_pts = Vec::new();
                for part in parts {
                    let v_idx_str = part.split('/').next().unwrap_or("");
                    if let Ok(idx) = v_idx_str.parse::<isize>() {
                        let actual_idx = if idx > 0 {
                            (idx - 1) as usize
                        } else if idx < 0 {
                            (file_verts.len() as isize + idx) as usize
                        } else {
                            continue;
                        };
                        if actual_idx < file_verts.len() {
                            face_pts.push(file_verts[actual_idx]);
                        }
                    }
                }

                if face_pts.len() >= 3 {
                    let mut loop_verts = Vec::new();
                    for pt in face_pts {
                        let v = scene.mesh.add_vertex(pt);
                        loop_verts.push(v);
                    }
                    scene.mesh.add_face(loop_verts, vec![]);
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// Save OBJ with a material sidecar and self-contained texture assets.
/// All source assets are read before touching the destination.
pub fn save_obj(scene: &Scene, path: &Path) -> Result<(), ObjError> {
    let triangles=crate::exchange::triangles(scene);
    let mut obj=Vec::new();let mut mtl=Vec::new();let mut assets=Vec::new();
    let stem=path.file_stem().and_then(|v|v.to_str()).unwrap_or("model");
    let mtl_path=path.with_extension("mtl");
    writeln!(obj,"# IngeTrazo OBJ Export\nmtllib {}",mtl_path.file_name().unwrap().to_string_lossy())?;
    for triangle in &triangles {for p in triangle.points {writeln!(obj,"v {:.6} {:.6} {:.6}",p.x,p.y,p.z)?;}}
    let mut uv_indices=Vec::new();let mut uv_count=0;
    for (i,t) in triangles.iter().enumerate() {
        let named=t.attrs.get("mat").and_then(|v|v.as_str());
        let registered=named.and_then(|name|scene.materials.get(name));
        let color=t.attrs.get("color").and_then(|v|v.as_array());
        let rgba=std::array::from_fn::<_,4,_>(|j|color.and_then(|c|c.get(j)).and_then(|v|v.as_f64()).unwrap_or_else(||registered.map_or([0.96,0.95,0.925,1.][j],|m|m.color[j] as f64)));
        let opacity=t.attrs.get("opacity").and_then(|v|v.as_f64()).unwrap_or_else(||registered.map_or(rgba[3],|m|m.opacity as f64));
        let label=named.unwrap_or("paint").chars().map(|c|if c.is_alphanumeric() || c=='_' {c} else {'_'}).collect::<String>();
        writeln!(mtl,"newmtl {label}_{i}\nKd {:.6} {:.6} {:.6}\nd {:.6}",rgba[0],rgba[1],rgba[2],opacity)?;
        let texture=t.attrs.get("texture").cloned().or_else(||registered.and_then(|m| {
            m.texture_embed.as_ref().map(|e|serde_json::json!({"embed":e})).or_else(||m.texture_path.as_ref().map(|p|serde_json::json!({"path":p})))
        }));
        if let Some(tex)=texture {
            let embed=tex.get("embed").and_then(|v|v.as_str());
            let source=tex.get("path").and_then(|v|v.as_str());
            let bytes=if let Some(data)=embed.and_then(|e|scene.embedded_textures.get(e)) {data.clone()}
                else if let Some(source)=source {std::fs::read(source)?}
                else {return Err(ObjError::Parse(0,"Texture has no readable embedded asset or path".into()));};
            let extension=Path::new(source.or(embed).unwrap_or("texture.png")).extension().and_then(|e|e.to_str()).filter(|e|e.chars().all(|c|c.is_ascii_alphanumeric())).unwrap_or("png");
            let filename=format!("{}_texture_{}.{}",stem.replace(['\n','\r'],"_"),uuid::Uuid::new_v4(),extension);
            writeln!(mtl,"map_Kd {filename}")?;assets.push((filename,bytes));
            let explicit=tex.get("uvw").and_then(|v|v.as_array()).filter(|a|a.len()==8);
            let [a,b,c]=t.points;let normal=(b-a).cross(c-a).normalize_or_zero();
            let (u,v)=uv_basis(normal);
            let angle=tex.get("rot").and_then(|v|v.as_f64()).unwrap_or(0.).to_radians();
            let (u,v)=(u*angle.cos()+v*angle.sin(),v*angle.cos()-u*angle.sin());
            let dimension=|key|tex.get(key).and_then(|v|v.as_f64()).filter(|v|v.abs()>1e-9).unwrap_or(1.);
            let first=uv_count+1;
            for k in 0..3 {
                let (uu,vv)=if let Some(map)=explicit {
                    let p=t.local_points[k];let n=|j:usize|map[j].as_f64().unwrap_or(0.);
                    (DVec3::new(n(0),n(1),n(2)).dot(p)+n(3),DVec3::new(n(4),n(5),n(6)).dot(p)+n(7))
                } else {(u.dot(t.points[k])/dimension("sw"),v.dot(t.points[k])/dimension("sh"))};
                if !uu.is_finite() || !vv.is_finite() {return Err(ObjError::Parse(0,"Nonfinite texture coordinates".into()));}
                writeln!(obj,"vt {:.9} {:.9}",uu,vv)?;uv_count+=1;
            }
            uv_indices.push(Some(first));
        } else {uv_indices.push(None);}
        writeln!(mtl)?;
    }
    for (i,t) in triangles.iter().enumerate() {
        let label=t.attrs.get("mat").and_then(|v|v.as_str()).unwrap_or("paint").chars().map(|c|if c.is_alphanumeric()||c=='_' {c} else {'_'}).collect::<String>();
        writeln!(obj,"usemtl {label}_{i}")?;
        let first=i*3+1;
        if let Some(uv)=uv_indices[i] {writeln!(obj,"f {first}/{uv} {}/{} {}/{}",first+1,uv+1,first+2,uv+2)?;}
        else {writeln!(obj,"f {first} {} {}",first+1,first+2)?;}
    }
    let parent=path.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or_else(||Path::new("."));
    for (name,data) in assets {let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(parent.join(name))?;file.write_all(&data)?;}
    std::fs::write(mtl_path,mtl)?;
    std::fs::write(path,obj)?;
    Ok(())
}

fn uv_basis(normal:DVec3)->(DVec3,DVec3) {
    let n=normal.normalize_or_zero();let length=n.x.hypot(n.y);
    if length<1e-3 {(DVec3::new(if n.z>=0. {1.} else {-1.},0.,0.),DVec3::Y)}
    else {let u=DVec3::new(-n.y/length,n.x/length,0.);(u,n.cross(u))}
}

pub fn load_obj(path: &Path) -> Result<Scene, ObjError> {
    let mut file = File::open(path)?;
    let mut scene = Scene::new();
    import_obj(&mut file, &mut scene)?;
    Ok(scene)
}

#[cfg(test)]
mod asset_tests {
    use super::*;
    use ingetrazo_core::{groups::Group,mesh::Mesh,materials::Material};
    #[test]
    fn material_texture_bytes_and_local_affine_uv_survive_nested_export() {
        let directory=std::env::temp_dir().join(format!("obj-assets-{}",uuid::Uuid::new_v4()));std::fs::create_dir(&directory).unwrap();
        let mut mesh=Mesh::new();let ids=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|mesh.add_vertex(p));
        let face=mesh.add_face(ids.to_vec(),vec![]).unwrap();
        mesh.faces[face].attrs.insert("mat".into(),serde_json::json!("Blue Paint"));
        mesh.faces[face].attrs.insert("texture".into(),serde_json::json!({"embed":"images/test.png","uvw":[1.,0.,0.,0.25,0.,1.,0.,0.5]}));
        let mut group=Group::with_mesh("mapped",mesh);group.transform=glam::DMat4::from_translation(DVec3::new(10.,20.,30.));
        let mut scene=Scene::new();scene.groups.push(group);scene.embedded_textures.insert("images/test.png".into(),vec![1,2,3,4]);
        scene.materials.insert("Blue Paint".into(),Material::new("Blue Paint",[0.1,0.2,0.8,0.6]));
        let path=directory.join("model.obj");save_obj(&scene,&path).unwrap();
        let text=std::fs::read_to_string(&path).unwrap();assert!(text.contains("vt 0.250000000 0.500000000"));assert!(text.contains("mtllib model.mtl"));assert!(text.contains("usemtl Blue_Paint_0"));
        let mtl=std::fs::read_to_string(directory.join("model.mtl")).unwrap();assert!(mtl.contains("Kd 0.100000 0.200000 0.800000"));assert!(mtl.contains("d 0.600000"));
        let image=mtl.lines().find_map(|l|l.strip_prefix("map_Kd ")).unwrap();assert_eq!(std::fs::read(directory.join(image)).unwrap(),vec![1,2,3,4]);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn missing_texture_fails_before_destination_or_sidecar_mutation() {
        let directory=std::env::temp_dir().join(format!("obj-assets-{}",uuid::Uuid::new_v4()));std::fs::create_dir(&directory).unwrap();
        let mut scene=Scene::new();let ids=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|scene.mesh.add_vertex(p));let f=scene.mesh.add_face(ids.to_vec(),vec![]).unwrap();
        scene.mesh.faces[f].attrs.insert("texture".into(),serde_json::json!({"embed":"missing"}));
        let path=directory.join("model.obj");std::fs::write(&path,b"original").unwrap();std::fs::write(path.with_extension("mtl"),b"material-original").unwrap();
        assert!(save_obj(&scene,&path).is_err());assert_eq!(std::fs::read(&path).unwrap(),b"original");assert_eq!(std::fs::read(path.with_extension("mtl")).unwrap(),b"material-original");
        std::fs::remove_dir_all(directory).unwrap();
    }
}
