use glam::{DVec3,DMat4};
use ingetrazo_core::{groups::Group,mesh::Mesh,scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::base::{ToolContext,ToolType};
use ingetrazo_core::math::Ray;
use serde_json::{Value,json};
fn mesh(m:&Mesh)->Value {json!(m.faces.iter().map(|f|json!({"vertices":f.vertices.iter().map(|&i|m.vertices[i].position.to_array()).collect::<Vec<_>>(),"holes":f.holes.iter().map(|h|h.iter().map(|&i|m.vertices[i].position.to_array()).collect::<Vec<_>>()).collect::<Vec<_>>(),"attrs":f.attrs})).collect::<Vec<_>>())}
fn groups(gs:&[Group])->Value {json!(gs.iter().map(|g|json!({"name":g.name,"faces":mesh(&g.mesh),"children":groups(&g.children),"transform":g.transform.to_cols_array()})).collect::<Vec<_>>())}
fn face(m:&mut Mesh,x:f64,y:f64,w:f64,h:f64)->usize {let ids=[DVec3::new(x,y,0.),DVec3::new(x+w,y,0.),DVec3::new(x+w,y+h,0.),DVec3::new(x,y+h,0.)].map(|p|m.add_vertex(p));m.add_face(ids.to_vec(),vec![]).unwrap()}
fn rect(a:&mut IngeTrazoApp,x:f64,y:f64){a.set_tool(ToolType::Rectangle);for p in [DVec3::new(x,y,0.),DVec3::new(x+2.,y+1.,0.)]{a.dispatch_tool_click(&ToolContext::new(p,Ray::new(p+DVec3::Z*10.,-DVec3::Z)));}}
fn snapshot(a:&mut IngeTrazoApp,root:&Mesh,phase:&str)->Value {
 let rows=a.bim_takeoff_rows().unwrap();let csv=ingetrazo_core::bim::quantities_csv(rows);let rows=rows.iter().map(|r|json!({"key":if matches!(r.key,ingetrazo_core::bim::ObjectKey::Faces(_)){"faces"}else{"group"},"class":r.class,"name":r.name,"area":r.area,"volume":r.volume})).collect::<Vec<_>>();
 json!({"phase":phase,"active":mesh(&a.scene.mesh),"loose":mesh(if a.component_edit_stack.is_empty(){&a.scene.mesh}else{root}),"groups":groups(&a.bim_document_groups()),"rows":rows,"csv":csv})
}
fn main(){let mut results=vec![];for name in ["classic-empty-root","classic-root-id40","placed-leaf-root-id40","nested-transformed-root-id40","linked-leaf-root-id40","tagged-mother-root-id40","tagged-group-root-id40"] {
 let mut a=IngeTrazoApp::new();
 if name!="classic-empty-root" {let id=face(&mut a.scene.mesh,20.,20.,2.,1.);a.scene.mesh.faces[id].attrs.insert("ifc".into(),json!({"id":40,"class":"IfcSlab","name":"root"}));}
 let root=a.scene.mesh.clone();let mut g=Group::new("target");
 if name.contains("tagged-group"){g.ifc_tag=Some(json!({"class":"IfcSlab","name":"target slab"}));}
 if name.contains("tagged-mother"){let id=face(&mut g.mesh,0.,0.,4.,4.);g.mesh.faces[id].attrs.extend(json!({"color":[1,0,0],"vendor":71,"ifc":{"id":99,"class":"IfcSlab","name":"mother"}}).as_object().unwrap().iter().map(|(k,v)|(k.clone(),v.clone())));}
 if name.contains("placed")||name.contains("linked"){g.transform=DMat4::from_translation(DVec3::new(4.,5.,0.));}
 if name.contains("linked"){a.scene.groups.push(g);let id=a.scene.make_component(a.scene.groups[0].id).unwrap();let sibling=a.scene.groups[0].duplicate_instance();a.scene.groups.push(sibling);assert!(a.scene.component_definitions.contains_key(&id));}
 else if name.contains("nested"){g.transform=DMat4::from_translation(DVec3::new(2.,0.,0.));let mut parent=Group::new("parent");parent.children.push(g);parent.transform=DMat4::from_translation(DVec3::new(4.,5.,0.));a.scene.groups.push(parent);}
 else {a.scene.groups.push(g);}
 a.bim_name_input="context".into();a.set_bim_draw_mode(true);a.scene.select(SelectionItem::Group(0));a.begin_component_edit();if name.contains("nested"){a.scene.select(SelectionItem::Group(0));a.begin_component_edit();}
 let mut phases=vec![snapshot(&mut a,&root,"entered")];rect(&mut a,0.,0.);phases.push(snapshot(&mut a,&root,"first"));rect(&mut a,0.,5.);phases.push(snapshot(&mut a,&root,"second"));a.undo();phases.push(snapshot(&mut a,&root,"undo"));a.redo();phases.push(snapshot(&mut a,&root,"redo"));while !a.component_edit_stack.is_empty(){a.finish_component_edit();assert!(!a.status_message.contains("refused"));}phases.push(snapshot(&mut a,&root,"finished"));results.push(json!({"case":name,"phases":phases}));
 }std::fs::write(std::env::args().nth(1).unwrap(),serde_json::to_vec_pretty(&json!({"recipes":results})).unwrap()).unwrap();}
