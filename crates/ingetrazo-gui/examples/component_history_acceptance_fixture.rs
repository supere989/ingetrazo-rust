use glam::{DMat4,DVec3};
use ingetrazo_core::{groups::Group,mesh::Mesh,scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
use serde_json::json;
fn mesh(y:f64)->Mesh {let mut m=Mesh::new();let v=[DVec3::new(20.,y,0.),DVec3::new(21.,y,0.),DVec3::new(20.,y+1.,0.)].map(|p|m.add_vertex(p));m.add_face(v.to_vec(),vec![]).unwrap();m}
fn add(app:&mut IngeTrazoApp,y:f64){app.save_undo_state();let v=[DVec3::new(20.,y,0.),DVec3::new(21.,y,0.),DVec3::new(20.,y+1.,0.)].map(|p|app.scene.mesh.add_vertex(p));app.scene.mesh.add_face(v.to_vec(),vec![]).unwrap();app.scene.bump_version();}
fn setup(kind:&str)->IngeTrazoApp {let mut app=IngeTrazoApp::new();app.scene=ingetrazo_core::scene::Scene::new();let mut g=Group::with_mesh(kind,mesh(-10.));
 if kind.starts_with("placed") || kind=="container" {g.transform=DMat4::from_translation(DVec3::new(3.,2.,1.));}
 if kind=="container"{g.children.push(Group::with_mesh("child",mesh(-20.)));}
 app.scene.groups.push(g);
 if kind=="placed-component" {let id=app.scene.groups[0].id;app.scene.make_component(id).unwrap();let mut peer=app.scene.groups[0].duplicate_instance();peer.transform=DMat4::from_translation(DVec3::X*10.)*peer.transform;app.scene.groups.push(peer);}
 if kind=="placed-ordinary" {let token=uuid::Uuid::new_v4();app.scene.register_mesh_share(token,app.scene.groups[0].mesh.clone()).unwrap();app.scene.groups[0].mesh_share=Some(token);let mut peer=app.scene.groups[0].duplicate_instance();peer.transform=DMat4::from_translation(DVec3::X*10.)*peer.transform;app.scene.groups.push(peer);}
 app.scene.select(SelectionItem::Group(0));app}
fn main(){let mut results=vec![];
for kind in ["classic","container","placed-component"]{
 let mut app=setup(kind);let ids=app.scene.groups.iter().map(|g|g.id).collect::<Vec<_>>();app.begin_component_edit();add(&mut app,0.);add(&mut app,3.);app.undo();let active_undo=app.scene.mesh.faces.len();app.redo();let active_redo=app.scene.mesh.faces.len();app.finish_component_edit();let exit_depth=app.undo_stack.len();app.undo();let exit_undo=app.scene.groups[0].mesh.faces.len();app.redo();results.push(json!({"name":format!("{kind}-local-two-undo-redo-exit"),"active_undo_faces":active_undo,"active_redo_faces":active_redo,"exit_depth":exit_depth,"exit_undo_faces":exit_undo,"exit_redo_faces":app.scene.groups[0].mesh.faces.len(),"ids_preserved":app.scene.groups.iter().map(|g|g.id).collect::<Vec<_>>()==ids}));
 let mut app=setup(kind);add(&mut app,0.);app.begin_component_edit();let active=app.scene.clone();app.undo();let active_stable=app.scene==active;let undo_depth=app.undo_stack.len();let redo_depth=app.redo_stack.len();app.redo();app.finish_component_edit();let exit_loose=app.scene.mesh.faces.len();app.undo();let loose_undo=app.scene.mesh.faces.len();app.redo();results.push(json!({"name":format!("{kind}-undo-prior-loose-command"),"active_stable":active_stable,"active_undo_depth":undo_depth,"active_redo_depth":redo_depth,"exit_loose_faces":exit_loose,"loose_undo_faces":loose_undo,"loose_redo_faces":app.scene.mesh.faces.len()}));
}
let mut app=setup("placed-ordinary");app.begin_component_edit();let before=app.scene.clone();let undo=app.undo_stack.clone();let redo=app.redo_stack.clone();app.undo();results.push(json!({"name":"native-safe-active-makeunique-refusal","scene_stable":app.scene==before,"stacks_stable":app.undo_stack==undo&&app.redo_stack==redo,"visible_refusal":app.status_message.contains("finish or cancel"),"context_depth":app.component_edit_stack.len()}));
let mut app=setup("placed-component");app.begin_component_edit();add(&mut app,0.);app.undo();app.finish_component_edit();let redo_depth=app.redo_stack.len();app.redo();let published=app.scene.groups.iter().all(|g|g.mesh.faces.len()==2);app.undo();results.push(json!({"name":"native-safe-unchanged-finish-redo-rebind","redo_retained":redo_depth==1,"both_placements_redone":published,"both_placements_undone":app.scene.groups.iter().all(|g|g.mesh.faces.len()==1)}));
println!("{}",serde_json::to_string(&results).unwrap());}
