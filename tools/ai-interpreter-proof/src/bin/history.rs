#[path="../owner_registry.rs"]mod owner_registry;
#[path="../owner_budget.rs"]mod owner_budget;
#[path="../journal.rs"] mod journal;
use journal::{MeshJournal,AppendUndo};use ingetrazo_core::scene::Scene;use glam::DVec3;
fn record(s:&Scene,u:&[AppendUndo],r:&[AppendUndo],name:&str,out:&mut Vec<serde_json::Value>){out.push(serde_json::json!({"name":name,"vertices":s.mesh.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"edges":s.mesh.edges.iter().map(|e|[e.v0,e.v1]).collect::<Vec<_>>(),"undo":u.len(),"redo":r.len()}));}
fn append(s:&mut Scene,u:&mut Vec<AppendUndo>,r:&mut Vec<AppendUndo>,a:f64,b:f64){let mut j=MeshJournal::begin(&s.mesh,s.version);j.add_edge(DVec3::new(a,0.,0.),DVec3::new(b,0.,0.)).unwrap();if let Some(c)=j.publish_owned(&mut s.mesh,s.version).unwrap(){s.bump_version();u.push(c);r.clear();}}
fn undo(s:&mut Scene,u:&mut Vec<AppendUndo>,r:&mut Vec<AppendUndo>){if let Some(mut c)=u.pop(){c.undo(&mut s.mesh).unwrap();s.bump_version();r.push(c);}}
fn redo(s:&mut Scene,u:&mut Vec<AppendUndo>,r:&mut Vec<AppendUndo>){if let Some(mut c)=r.pop(){c.redo(&mut s.mesh).unwrap();s.bump_version();u.push(c);}}
fn main(){let mut s=Scene::new();let mut u=vec![];let mut r=vec![];let mut out=vec![];append(&mut s,&mut u,&mut r,0.,1.);record(&s,&u,&r,"append",&mut out);undo(&mut s,&mut u,&mut r);record(&s,&u,&r,"undo",&mut out);
let j=MeshJournal::begin(&s.mesh,s.version);assert!(j.publish_owned(&mut s.mesh,s.version).unwrap().is_none());record(&s,&u,&r,"noop-with-redo",&mut out);
let mut j=MeshJournal::begin(&s.mesh,s.version);j.add_edge(DVec3::new(8.,0.,0.),DVec3::new(9.,0.,0.)).unwrap();drop(j);record(&s,&u,&r,"failure-with-redo",&mut out);
redo(&mut s,&mut u,&mut r);record(&s,&u,&r,"redo",&mut out);for i in 0..2{undo(&mut s,&mut u,&mut r);record(&s,&u,&r,&format!("repeat-undo-{i}"),&mut out);redo(&mut s,&mut u,&mut r);record(&s,&u,&r,&format!("repeat-redo-{i}"),&mut out);}undo(&mut s,&mut u,&mut r);record(&s,&u,&r,"branch-undo",&mut out);append(&mut s,&mut u,&mut r,3.,4.);record(&s,&u,&r,"branch-new",&mut out);redo(&mut s,&mut u,&mut r);record(&s,&u,&r,"branch-redo-refused",&mut out);println!("{}",serde_json::to_string_pretty(&out).unwrap());}
