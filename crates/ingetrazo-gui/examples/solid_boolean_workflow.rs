//! Actual application publication/history; output meshes independently checked by Python.
use glam::{DVec3,DMat4};
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_core::{mesh::Mesh,groups::Group,scene::SelectionItem,solids::SolidOp};
use serde_json::json;
fn cube(name:&str)->Group {
 let mut m=Mesh::new();for p in [[0.,0.,0.],[1.,0.,0.],[1.,1.,0.],[0.,1.,0.],[0.,0.,1.],[1.,0.,1.],[1.,1.,1.],[0.,1.,1.]]{m.add_vertex(DVec3::from_array(p));}
 for f in [[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]]{let i=m.add_face(f.to_vec(),vec![]).unwrap();m.faces[i].attrs.insert("owner".into(),json!(name));m.faces[i].attrs.insert("future".into(),json!({"opaque":[1,2]}));}
 Group::with_mesh(name,m)
}
fn main(){let mut rows=Vec::new();
 for scenario in ["overlap","disjoint","transformed"] {for (op,expected) in [(SolidOp::Union,1.5),(SolidOp::Subtract,0.5),(SolidOp::Intersect,0.5)] {
 let mut app=IngeTrazoApp::new();let a=cube("a");let mut b=cube("b");
 b.transform=if scenario=="disjoint"{DMat4::from_translation(DVec3::new(3.,0.,0.))}else if scenario=="transformed"{DMat4::from_translation(DVec3::new(1.5,0.,0.))*DMat4::from_rotation_z(std::f64::consts::FRAC_PI_2)}else{DMat4::from_translation(DVec3::new(0.5,0.,0.))};
 app.scene.groups=vec![a,b];app.scene.select(SelectionItem::Group(0));app.scene.select(SelectionItem::Group(1));let before=app.scene.clone();app.perform_solid_boolean(op);
 let refused=scenario=="disjoint" && op!=SolidOp::Union;
 if refused{assert_eq!(app.scene,before);assert!(app.undo_stack.is_empty());assert!(app.redo_stack.is_empty());assert!(app.status_message.starts_with("Solid error:"));}
 else{assert_eq!(app.undo_stack.len(),1);assert_eq!(app.scene.groups.len(),1);let after=app.scene.clone();app.undo();assert_eq!(app.scene.groups,before.groups);assert_eq!(app.scene.selection,before.selection);app.redo();assert_eq!(app.scene.groups,after.groups);assert_eq!(app.scene.selection,after.selection);}
 let meshes=app.scene.groups.iter().map(|g|json!({"transform":g.transform.to_cols_array(),"vertices":g.mesh.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"faces":g.mesh.faces.iter().map(|f|json!({"outer":f.vertices,"holes":f.holes,"attrs":f.attrs})).collect::<Vec<_>>() })).collect::<Vec<_>>();
 rows.push(json!({"scenario":scenario,"operation":op.name(),"expected_volume":if scenario=="disjoint"{2.0}else{expected},"refused":refused,"history_restored":!refused,"unchanged_refusal":refused,"meshes":meshes}));
 }}
 // Malformed transformed input must preserve all application state/history.
 let mut app=IngeTrazoApp::new();app.scene.groups=vec![cube("a"),cube("b")];app.scene.groups[1].transform=DMat4::from_scale(DVec3::ZERO);app.scene.select(SelectionItem::Group(0));app.scene.select(SelectionItem::Group(1));let before=app.scene.clone();app.perform_solid_boolean(SolidOp::Union);assert_eq!(app.scene,before);assert!(app.undo_stack.is_empty());assert!(app.redo_stack.is_empty());
 println!("{}",json!({"cases":rows,"invalid_transform_atomic":true,"scope":"actual app handler, no desktop event/render claim"}));
}
