use glam::DVec3;
use ingetrazo_core::{math::Ray,scene::{Scene,SelectionItem},units};
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::base::{ToolContext,ToolType};
use serde_json::{json,Value};
use std::path::Path;
fn ctx(p:DVec3)->ToolContext{ToolContext::new(p,Ray::new(p+DVec3::Z*5.,-DVec3::Z))}
fn app(unit:&str)->IngeTrazoApp{
 let mut a=IngeTrazoApp::new();a.scene=Scene::new();
 a.scene.unit=match unit{"cm"=>units::LengthUnit::Centimeters,"mm"=>units::LengthUnit::Millimeters,"in"=>units::LengthUnit::Inches,"ft"=>units::LengthUnit::Feet,_=>units::LengthUnit::Meters};
 a.scene.native_document["units"]=json!({"length":unit,"precision":2});a
}
fn state(a:&IngeTrazoApp)->Value{
 json!({"mesh":super::state(&a.scene.mesh),"guides":a.scene.guides.iter().map(|g|json!({"kind":g.kind,"origin":g.origin})).collect::<Vec<_>>(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len()})
}
fn workflow(name:&str,unit:&str,text:&str)->Value{
 let mut a=app(unit);
 let tool=match name{"line"|"line-delta"=>ToolType::Line,"rectangle"=>ToolType::Rectangle,"move"|"array"=>ToolType::Move,"pushpull"=>ToolType::PushPull,"offset"=>ToolType::Offset,"fillet"=>ToolType::Fillet,"tape"=>ToolType::Tape,"arc"=>ToolType::Arc,"circle"|"circle-count"=>ToolType::Circle,"rotate"=>ToolType::Rotate,"scale"|"scale-absolute"=>ToolType::Scale,_=>panic!("Unknown case")};
 if matches!(name,"move"|"array"|"pushpull"|"offset"|"fillet"|"rotate"|"scale"|"scale-absolute"){
  let ids=[DVec3::ZERO,DVec3::X,DVec3::new(1.,1.,0.),DVec3::Y].into_iter().map(|p|a.scene.mesh.add_vertex(p)).collect();a.scene.mesh.add_face(ids,vec![]);
  if matches!(name,"move"|"array"|"rotate"|"scale"|"scale-absolute"){a.scene.select(SelectionItem::Face(0));}
 }
 if name=="fillet"{ingetrazo_tools::pushpull::PushPullTool::extrude_face(&mut a.scene,0,1.).unwrap();}
 a.set_tool(tool);
 match name{
  "line"|"line-delta"|"rectangle"|"tape"|"circle"=>{a.dispatch_tool_click(&ctx(DVec3::ZERO));a.dispatch_tool_move(&ctx(DVec3::new(1.,1.,0.)));},
  "circle-count"=>{},
  "arc"=>{a.dispatch_tool_click(&ctx(DVec3::ZERO));a.dispatch_tool_click(&ctx(DVec3::X*2.));a.dispatch_tool_move(&ctx(DVec3::new(1.,1.,0.)));},
  "move"|"array"=>{let mut c=ctx(DVec3::ZERO);c.ctrl_held=name=="array";a.dispatch_tool_click(&c);a.dispatch_tool_move(&ctx(DVec3::X));},
  "pushpull"|"offset"=>a.dispatch_tool_click(&ctx(DVec3::new(0.5,0.5,0.))),
  "fillet"=>{let c=ToolContext::new(DVec3::new(0.5,0.,1.),Ray::new(DVec3::new(0.5,-2.,1.),DVec3::Y));a.dispatch_tool_move(&c);a.dispatch_tool_click(&c);},
  "rotate"=>{a.dispatch_tool_click(&ctx(DVec3::ZERO));a.dispatch_tool_click(&ctx(DVec3::X));},
  "scale"|"scale-absolute"=>{a.dispatch_tool_click(&ctx(DVec3::new(1.,0.5,0.)));},
  _=>unreachable!()
 }
 let before=state(&a);a.dispatch_tool_vcb(text).unwrap_or_else(|e|panic!("{name}/{unit}/{text}: {e}"));
 if name=="circle-count"{a.dispatch_tool_click(&ctx(DVec3::ZERO));a.dispatch_tool_move(&ctx(DVec3::X));a.dispatch_tool_click(&ctx(DVec3::X));}
 let committed=state(&a);assert_ne!(committed,before,"{name} did not change app");
 a.dispatch_tool_cancel();a.undo();let undone=state(&a);a.redo();let redone=state(&a);
 json!({"before":before,"commit":committed,"undo":undone,"redo":redone})
}
pub fn run(out:&Path){
 let source:Value=serde_json::from_slice(&std::fs::read(out.join("original-measurements.json")).unwrap()).unwrap();
 let mut rows=vec![];
 for row in source["rows"].as_array().unwrap(){
  let a=app(row["unit"].as_str().unwrap());let text=row["text"].as_str().unwrap();
  let value=if row["mode"]=="angle"{units::vcb::parse_angle(text).ok().map_or(Value::Null,|v|json!(v))}else{
   units::vcb::parse_values(text,if row["unitless"].as_bool().unwrap(){1.}else{units::model_scale(&a.scene)},row["comma_lists"].as_bool().unwrap()).ok().map_or(Value::Null,|v|json!(v))};
  let mut native=row.clone();native["value"]=value;rows.push(native);
 }
 std::fs::write(out.join("native-measurements.json"),serde_json::to_vec_pretty(&json!({"rows":rows})).unwrap()).unwrap();
 let cases=[("line","500","0.5m"),("line-delta","500;250;0","0.5m;0.25m;0m"),("rectangle","500,250","0.5m;0.25m"),("move","500","0.5m"),("array","3x500","3x0.5m"),("pushpull","500","0.5m"),("offset","125","0.125m"),("fillet","125","0.125m"),("tape","500","0.5m"),("arc","500","0.5m"),("circle","500","0.5m"),("circle-count","6,5","6.5"),("rotate","45,5","45.5"),("scale","1,5","1.5"),("scale-absolute","500mm","0.5m")];
 let mut workflows=vec![];
 for(name,bare,explicit)in cases{
  let actual=workflow(name,"mm",bare);let reference=workflow(name,"m",explicit);
  assert_eq!(actual,reference,"Document-unit routing {name}");workflows.push(json!({"case":name,"typed_mm":bare,"control_m":explicit,"states":actual}));
 }
 std::fs::write(out.join("app-measurement-workflows.json"),serde_json::to_vec_pretty(&json!({"workflows":workflows,"scope":"Actual App unit-routing equality against explicit physical controls; independent source numeric grammar is separate. No fresh whole-tool source geometry claim."})).unwrap()).unwrap();
}
