//! Read-only part measurements and merged cut lists, matching core/parts.py.
use glam::{DMat4,DVec3};use crate::groups::Group;
#[derive(Debug,Clone,PartialEq)]pub struct PartSummary{pub id:uuid::Uuid,pub name:String,pub material:String,pub size:[f64;3],pub faces:usize,pub hidden:bool}
#[derive(Debug,Clone,PartialEq)]pub struct CutListLine{pub names:Vec<String>,pub materials:Vec<String>,pub size:[f64;3],pub quantity:usize}
pub fn part_points(group:&Group)->Vec<DVec3>{fn walk(g:&Group,parent:DMat4,out:&mut Vec<DVec3>){let m=parent*g.transform;out.extend(g.mesh.vertices.iter().map(|v|m.transform_point3(v.position)));for child in &g.children{walk(child,m,out);}}let mut points=vec![];walk(group,DMat4::IDENTITY,&mut points);points}
pub fn part_size(points:&[DVec3])->[f64;3]{
    if points.is_empty(){return[0.;3];}
    fn extents(points:&[DVec3],axes:[DVec3;3])->[f64;3]{let mut lo=[f64::INFINITY;3];let mut hi=[f64::NEG_INFINITY;3];for p in points{for i in 0..3{let q=p.dot(axes[i]);lo[i]=lo[i].min(q);hi[i]=hi[i].max(q);}}[hi[0]-lo[0],hi[1]-lo[1],hi[2]-lo[2]]}
    let mut best=extents(points,[DVec3::X,DVec3::Y,DVec3::Z]);
    if points.len()>=4{
        let mean=points.iter().copied().sum::<DVec3>()/points.len() as f64;let mut covariance=[[0.;3];3];let mut vectors=[[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]];
        for point in points{let p=(*point-mean).to_array();for i in 0..3{for j in 0..3{covariance[i][j]+=p[i]*p[j];}}}
        // Jacobi rotations diagonalize the symmetric covariance without a
        // dependency on a BLAS runtime. Axes remain orthonormal.
        for _ in 0..64{let mut pair=(0,1);for(i,j)in[(0,2),(1,2)]{if covariance[i][j].abs()>covariance[pair.0][pair.1].abs(){pair=(i,j);}}
            let(p,q)=pair;if covariance[p][q].abs()<1e-14*(1.+covariance[0][0].abs()+covariance[1][1].abs()+covariance[2][2].abs()){break;}
            let angle=0.5*(2.*covariance[p][q]).atan2(covariance[q][q]-covariance[p][p]);let(c,s)=(angle.cos(),angle.sin());
            let app=covariance[p][p];let aqq=covariance[q][q];let apq=covariance[p][q];
            covariance[p][p]=c*c*app-2.*c*s*apq+s*s*aqq;covariance[q][q]=s*s*app+2.*c*s*apq+c*c*aqq;covariance[p][q]=0.;covariance[q][p]=0.;
            for k in 0..3{if k!=p&&k!=q{let a=covariance[k][p];let b=covariance[k][q];covariance[k][p]=c*a-s*b;covariance[p][k]=covariance[k][p];covariance[k][q]=s*a+c*b;covariance[q][k]=covariance[k][q];}let a=vectors[k][p];let b=vectors[k][q];vectors[k][p]=c*a-s*b;vectors[k][q]=s*a+c*b;}
        }
        let axes=[DVec3::new(vectors[0][0],vectors[1][0],vectors[2][0]),DVec3::new(vectors[0][1],vectors[1][1],vectors[2][1]),DVec3::new(vectors[0][2],vectors[1][2],vectors[2][2])];let own=extents(points,axes);
        if own.iter().product::<f64>()<0.8*best.iter().product::<f64>(){best=own;}
    }
    best.sort_by(|a,b|b.total_cmp(a));best
}
fn material_label(attrs:&serde_json::Value)->Option<String>{
    if let Some(name)=attrs.get("mat").and_then(|v|v.as_str()).filter(|s|!s.is_empty()){return Some(name.to_owned());}
    if let Some(path)=attrs.get("texture").and_then(|v|v.get("path")).and_then(|v|v.as_str()){return std::path::Path::new(path).file_stem().map(|s|s.to_string_lossy().to_string());}
    let color=attrs.get("color")?.as_array()?;if color.len()<3{return None;}let mut rgb=[0u8;3];for i in 0..3{rgb[i]=(color[i].as_f64()?*255.).round_ties_even().clamp(0.,255.) as u8;}Some(format!("#{:02x}{:02x}{:02x}",rgb[0],rgb[1],rgb[2]))
}
pub fn part_summary(group:&Group)->PartSummary{
    fn walk(g:&Group,paint:Option<&serde_json::Value>,areas:&mut Vec<(String,f64)>,faces:&mut usize){
        let inherited=g.native_document.get("material").filter(|v|!v.is_null()).or(paint);
        for(i,face)in g.mesh.faces.iter().enumerate(){let own=serde_json::to_value(&face.attrs).unwrap();let label=material_label(&own).or_else(||inherited.and_then(material_label));if let Some(label)=label{let area=g.mesh.face_area(i);if let Some((_,total))=areas.iter_mut().find(|(name,_)|name==&label){*total+=area;}else{areas.push((label,area));}}*faces+=1;}
        // Original part_material uses each placement paint or the queried
        // root part paint; it does not inherit intermediate ancestors.
        for child in &g.children{walk(child,paint,areas,faces);}
    }
    let mut areas=vec![];let mut faces=0;walk(group,group.native_document.get("material").filter(|v|!v.is_null()),&mut areas,&mut faces);
    let material=areas.into_iter().reduce(|a,b|if b.1>a.1{b}else{a}).map(|(name,_)|name).unwrap_or_default();
    PartSummary{id:group.id,name:group.name.clone(),material,size:part_size(&part_points(group)),faces,hidden:group.hidden}
}
fn natural_compare(a:&str,b:&str)->std::cmp::Ordering {
    fn tokens(s:&str)->Vec<String>{let mut tokens=vec![];let mut current=String::new();let mut digit=None;for c in s.to_lowercase().chars(){let is_digit=c.is_ascii_digit();if digit.is_some()&&digit!=Some(is_digit){tokens.push(std::mem::take(&mut current));}current.push(c);digit=Some(is_digit);}if !current.is_empty(){tokens.push(current);}tokens}
    let a=tokens(a);let b=tokens(b);for(x,y)in a.iter().zip(&b){let numeric=x.as_bytes()[0].is_ascii_digit()&&y.as_bytes()[0].is_ascii_digit();let cmp=if numeric{let x=x.trim_start_matches('0');let y=y.trim_start_matches('0');x.len().cmp(&y.len()).then_with(||x.cmp(y))}else{x.cmp(y)};if cmp!=std::cmp::Ordering::Equal{return cmp;}}a.len().cmp(&b.len())
}
pub fn cut_list(rows:&[PartSummary],precision_mm:f64,by_material:bool)->Vec<CutListLine>{
    let precision=if precision_mm.is_finite()&&precision_mm>0.{precision_mm}else{1.};let mut lines:Vec<CutListLine>=vec![];
    for row in rows{let key=row.size.map(|v|(v*1000./precision).round_ties_even());let found=lines.iter_mut().find(|line|line.size.map(|v|(v*1000./precision).round_ties_even())==key&&(!by_material||line.materials[0]==row.material));
        if let Some(line)=found{line.quantity+=1;line.names.push(row.name.clone());if !line.materials.contains(&row.material){line.materials.push(row.material.clone());}}else{lines.push(CutListLine{names:vec![row.name.clone()],materials:vec![row.material.clone()],size:row.size,quantity:1});}
    }
    for line in &mut lines{line.names.sort_by(|a,b|natural_compare(a,b));}lines.sort_by(|a,b|{for i in 0..3{let cmp=(b.size[i]*1e6).round_ties_even().total_cmp(&(a.size[i]*1e6).round_ties_even());if cmp!=std::cmp::Ordering::Equal{return cmp;}}natural_compare(&a.names[0],&b.names[0])});lines
}
pub fn cut_list_text(lines:&[CutListLine])->String{let mut text="Qty\tParts\tMaterial\tL (mm)\tW (mm)\tT (mm)".to_owned();for line in lines{text.push_str(&format!("\n{}\t{}\t{}\t{:.0}\t{:.0}\t{:.0}",line.quantity,line.names.join(", "),line.materials.iter().filter(|s|!s.is_empty()).cloned().collect::<Vec<_>>().join(", "),line.size[0]*1000.,line.size[1]*1000.,line.size[2]*1000.));}text}
#[cfg(test)]mod tests{use super::*;
    fn box_points(size:DVec3)->Vec<DVec3>{let mut p=vec![];for x in[0.,1.]{for y in[0.,1.]{for z in[0.,1.]{p.push(size*DVec3::new(x,y,z));}}}p}
    #[test]fn nested_material_matches_original_root_fallback_and_explicit_face_override(){
        let mut mesh=crate::mesh::Mesh::new();let ids=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|mesh.add_vertex(p));mesh.add_face(ids.to_vec(),vec![]);let leaf=Group::with_mesh("leaf",mesh);let mut middle=Group::new("middle");middle.native_document=serde_json::json!({"material":{"mat":"middle paint"}});middle.children.push(leaf);let mut root=Group::new("root");root.native_document=serde_json::json!({"material":{"mat":"root paint"}});root.children.push(middle);
        assert_eq!(part_summary(&root).material,"root paint");root.children[0].children[0].mesh.faces[0].attrs.insert("mat".into(),serde_json::json!("face paint"));assert_eq!(part_summary(&root).material,"face paint");root.children[0].native_document=serde_json::Value::Null;root.children[0].children[0].mesh.faces[0].attrs.clear();assert_eq!(part_summary(&root).material,"root paint");
    }
    #[test]fn surface_majority_material_uses_explicit_paint_then_inherited_container(){
        let mut mesh=crate::mesh::Mesh::new();let a=mesh.add_vertex(DVec3::ZERO);let b=mesh.add_vertex(DVec3::X);let c=mesh.add_vertex(DVec3::Y);mesh.add_face(vec![a,b,c],vec![]);
        let a=mesh.add_vertex(DVec3::new(2.,0.,0.));let b=mesh.add_vertex(DVec3::new(2.1,0.,0.));let c=mesh.add_vertex(DVec3::new(2.,0.1,0.));let small=mesh.add_face(vec![a,b,c],vec![]).unwrap();mesh.faces[small].attrs.insert("color".into(),serde_json::json!([1.,0.,0.]));
        let mut part=Group::with_mesh("painted",mesh);part.native_document=serde_json::json!({"material":{"color":[0.,1.,0.]}});assert_eq!(part_summary(&part).material,"#00ff00");part.mesh.faces[0].attrs.insert("mat".into(),serde_json::json!("named wood"));assert_eq!(part_summary(&part).material,"named wood");
    }
    #[test]fn cut_list_merges_size_material_and_sorts_names_naturally(){
        let row=PartSummary{id:uuid::Uuid::new_v4(),name:"Piece 10".into(),material:"wood".into(),size:[0.58,0.04,0.04],faces:6,hidden:false};let mut copy=row.clone();copy.name="Piece 2".into();let lines=cut_list(&[row.clone(),copy.clone()],1.,true);assert_eq!(lines.len(),1);assert_eq!(lines[0].quantity,2);assert_eq!(lines[0].names,vec!["Piece 2","Piece 10"]);assert!(cut_list_text(&lines).contains("2\tPiece 2, Piece 10\twood\t580\t40\t40"));copy.material="metal".into();assert_eq!(cut_list(&[row.clone(),copy.clone()],1.,true).len(),2);assert_eq!(cut_list(&[row,copy],1.,false).len(),1);
    }
    #[test]fn part_principal_box_matches_splayed_leg_but_retains_stepped_carcass_model_axes(){let turn=DMat4::from_rotation_y(30_f64.to_radians());let p=box_points(DVec3::new(0.04,0.04,0.58)).into_iter().map(|p|turn.transform_point3(p)).collect::<Vec<_>>();let size=part_size(&p);for(a,b)in size.into_iter().zip([0.58,0.04,0.04]){assert!((a-b).abs()<1e-8);}
        let mut points=box_points(DVec3::new(0.442,0.612,0.709)).into_iter().map(|p|p+DVec3::new(0.,-0.052,0.085)).collect::<Vec<_>>();points.extend(box_points(DVec3::new(0.442,0.56,0.085)));for(a,b)in part_size(&points).into_iter().zip([0.794,0.612,0.442]){assert!((a-b).abs()<1e-8);}}
}
