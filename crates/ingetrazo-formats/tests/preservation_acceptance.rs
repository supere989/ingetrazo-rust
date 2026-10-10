//! Independently authored editable-document regression, beyond no-op raw retention.
use glam::{DMat4,DVec3};
use ingetrazo_formats::igz::{load_igz,scene_to_igz_document};
use serde_json::json;
#[test]
fn edited_geometry_material_layer_and_group_replace_raw_baseline() {
    let path=std::env::temp_dir().join(format!("parity-independent-{}.igz",uuid::Uuid::new_v4()));
    let original=json!({"faces":[{"vertices":[[0,0,0],[1,0,0],[0,1,0]],"audit_extension":{"opaque":17}}],"edges":[],
        "groups":[{"name":"original","faces":[{"vertices":[[0,0,0],[1,0,0],[0,1,0]]}],"xform":[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1],"audit_group":19}],
        "layers":[{"name":"audit","visible":true,"locked":false}],"materials":[{"name":"paint","color":[1,0,0],"audit_material":21}],"audit_document":{"opaque":[23]}});
    std::fs::write(&path,serde_json::to_vec(&json!({"igz_format":1,"app_version":"audit","scene":original})).unwrap()).unwrap();
    let mut scene=load_igz(&path).unwrap();
    scene.mesh.vertices[0].position.z=2.;
    scene.groups[0].name="changed".into();scene.groups[0].transform=DMat4::from_translation(DVec3::new(4.,5.,6.));
    scene.layers.iter_mut().find(|l|l.name=="audit").unwrap().locked=true;
    scene.materials.get_mut("paint").unwrap().color=[0.,1.,0.,1.];
    let out=scene_to_igz_document(&scene);
    assert_eq!(out["faces"][0]["vertices"][0][2],2.);
    assert_eq!(out["faces"][0]["audit_extension"],original["faces"][0]["audit_extension"]);
    assert_eq!(out["groups"][0]["name"],"changed");assert_eq!(out["groups"][0]["xform"][12],4.);
    assert_eq!(out["groups"][0]["audit_group"],19);
    assert_eq!(out["layers"][0]["locked"],true);
    assert_eq!(out["materials"][0]["color"][1],1.);
    assert_eq!(out["materials"][0]["audit_material"],21);
    assert_eq!(out["audit_document"],original["audit_document"]);
    scene.mesh.faces.clear();assert_eq!(scene_to_igz_document(&scene)["faces"].as_array().unwrap().len(),0);
    let ids=[[0.,0.,7.],[2.,0.,7.],[0.,2.,7.]].map(|p|scene.mesh.add_vertex(DVec3::from_array(p)));scene.mesh.add_face(ids.to_vec(),vec![]).unwrap();
    let out=scene_to_igz_document(&scene);assert_eq!(out["faces"].as_array().unwrap().len(),1);assert_eq!(out["faces"][0]["vertices"][0][2],7.);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn renamed_material_retains_unknown_source_attributes() {
    let path=std::env::temp_dir().join(format!("parity-rename-{}.igz",uuid::Uuid::new_v4()));
    std::fs::write(&path,serde_json::to_vec(&json!({"igz_format":1,"app_version":"audit","scene":{"faces":[],"edges":[],"materials":[{"name":"paint","color":[1,0,0],"unknown_projection":{"tile":[2,3]}}]}})).unwrap()).unwrap();
    let mut scene=load_igz(&path).unwrap();let mut material=scene.materials.remove("paint").unwrap();material.name="renamed".into();scene.materials.insert("renamed".into(),material);
    let out=scene_to_igz_document(&scene);std::fs::remove_file(path).unwrap();
    assert_eq!(out["materials"][0]["name"],"renamed");assert_eq!(out["materials"][0]["unknown_projection"],json!({"tile":[2,3]}));
}
#[test]
fn deleting_edge_does_not_strip_surviving_curve_metadata() {
    let path=std::env::temp_dir().join(format!("parity-edge-{}.igz",uuid::Uuid::new_v4()));
    std::fs::write(&path,serde_json::to_vec(&json!({"igz_format":1,"app_version":"audit","scene":{"faces":[],"edges":[{"a":[0,0,0],"b":[1,0,0],"curve":{"id":"first"}},{"a":[0,2,0],"b":[1,2,0],"curve":{"id":"second"}}]}})).unwrap()).unwrap();
    let mut scene=load_igz(&path).unwrap();scene.mesh.edges.remove(0);
    let out=scene_to_igz_document(&scene);std::fs::remove_file(path).unwrap();assert_eq!(out["edges"][0]["curve"],json!({"id":"second"}));
}
