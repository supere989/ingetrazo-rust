use glam::{DMat4, DVec3};
use ingetrazo_formats::igz::{load_igz, save_igz, scene_to_igz_document};
use serde_json::{json, Value};
use std::io::{Read, Write};

fn native() -> Value {
    json!({
        "edges": [{"a":[0,0,0], "b":[1,0,0], "curve":{"future":"arc"}}],
        "faces": [],
        "protos": [{"edges":[], "faces":[{"vertices":[[0,0,0],[1,0,0],[0,1,0]],"material":{"name":"paint"},"future_face":{"v":1}}]}],
        "groups": [{"name":"parent","proto":0,"component":true,
            "xform":[1,0,0,0,0,1,0,0,0,0,1,0,4,5,6,1],
            "ext":{"opaque":[1,{"v":true}]}, "layer":"BIM", "ifc":{"class":"IfcWall"},
            "children":[{"name":"nested","proto":0,"xform":[1,0,0,0,0,1,0,0,0,0,1,0,0,0,2,1],"uid":"stable"}]}],
        "materials":[{"name":"paint","color":[0.1,0.2,0.3], "texture":{"embed":"textures/paint.bin","tile":[2,3]}}],
        "saved_views":[{"name":"saved","future_view":{"x":1}}],
        "compositions":[{"name":"sheet","unknown_layout":[1,2]}],
        "section_planes":[{"name":"section","future_plane":[0,1]}],
        "section_view":{"planes_hidden":true,"cuts_hidden":true},
        "georef":{"datum":{"future_datum":"opaque"}},
        "plugin_data":{"future_plugin":{"nested":[1,false,null]}},
        "units":{"length":"mm","precision":4}
    })
}

fn temp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("igz-{}-{name}", uuid::Uuid::new_v4()))
}
fn write_plain(doc: &Value, path: &std::path::Path) {
    std::fs::write(path, serde_json::to_vec(&json!({"igz_format":1,"app_version":"original","scene":doc})).unwrap()).unwrap();
}

#[test]
fn native_noop_preserves_every_nested_value_and_repeated_save() {
    let src=temp("source.igz"); let dst=temp("saved.igz");
    let original=native(); write_plain(&original,&src);
    let scene=load_igz(&src).unwrap();
    assert_eq!(scene.groups[0].children.len(),1);
    assert_eq!(scene.groups[0].mesh.faces.len(),1);
    assert_eq!(scene.groups[0].layer.as_deref(),Some("BIM"));
    assert!(!scene.show_section_cuts);
    assert_eq!(scene_to_igz_document(&scene),original);
    save_igz(&scene,&dst).unwrap();
    assert_eq!(scene_to_igz_document(&load_igz(&dst).unwrap()),original);
    std::fs::remove_file(src).unwrap(); std::fs::remove_file(dst).unwrap();
}

#[test]
fn edited_nested_instance_detaches_geometry_preserving_original_definition_and_metadata() {
    let src=temp("edit-source.igz"); let dst=temp("edit-save.igz");
    let original=native(); write_plain(&original,&src);
    let mut scene=load_igz(&src).unwrap();
    let child=&mut scene.groups[0].children[0];
    child.name="edited".into();
    child.transform=DMat4::from_translation(DVec3::new(7.,8.,9.));
    child.mesh.vertices[0].position.z=3.;
    let document=scene_to_igz_document(&scene);
    assert_eq!(document["protos"][0],original["protos"][0]);
    assert_eq!(document["groups"][0]["children"][0]["proto"],1);
    assert_eq!(document["groups"][0]["children"][0]["uid"],"stable");
    assert_eq!(document["groups"][0]["ext"],original["groups"][0]["ext"]);
    assert_eq!(document["plugin_data"],original["plugin_data"]);
    save_igz(&scene,&dst).unwrap();
    let reopened=load_igz(&dst).unwrap();
    assert_eq!(reopened.groups[0].children[0].mesh.vertices[0].position.z,3.);
    assert_eq!(reopened.groups[0].mesh.vertices[0].position.z,0.);
    assert_eq!(reopened.groups[0].children[0].transform,scene.groups[0].children[0].transform);
    std::fs::remove_file(src).unwrap(); std::fs::remove_file(dst).unwrap();
}

#[test]
fn zip_embedded_texture_and_survey_bytes_survive_without_disk_files() {
    let src=temp("zip-source.igz"); let dst=temp("zip-save.igz");
    let mut archive=zip::ZipWriter::new(std::fs::File::create(&src).unwrap());
    let options=zip::write::SimpleFileOptions::default();
    for (name,bytes) in [("textures/paint.bin",vec![0,255,1,2]),("survey/mesh.bin",vec![7,8,9])] {
        archive.start_file(name,options).unwrap(); archive.write_all(&bytes).unwrap();
    }
    archive.start_file("document.json",options).unwrap();
    archive.write_all(&serde_json::to_vec(&json!({"igz_format":2,"app_version":"original","scene":native()})).unwrap()).unwrap();
    archive.finish().unwrap();
    let scene=load_igz(&src).unwrap(); save_igz(&scene,&dst).unwrap();
    let mut reopened=zip::ZipArchive::new(std::fs::File::open(&dst).unwrap()).unwrap();
    for (name,expected) in [("textures/paint.bin",vec![0,255,1,2]),("survey/mesh.bin",vec![7,8,9])] {
        let mut actual=vec![]; reopened.by_name(name).unwrap().read_to_end(&mut actual).unwrap();
        assert_eq!(actual,expected);
    }
    assert_eq!(scene_to_igz_document(&load_igz(&dst).unwrap()),native());
    std::fs::remove_file(src).unwrap(); std::fs::remove_file(dst).unwrap();
}


#[test]
fn native_camera_section_and_datum_edits_use_original_schema() {
    let src=temp("schema-source.igz"); let dst=temp("schema-save.igz");
    let doc=json!({
        "edges":[],"faces":[],
        "saved_views":[{"name":"view","fov":62.,"parallel":true,"style":{"opaque":17}}],
        "section_planes":[{"point":[1.,2.,3.],"normal":[0.,0.,1.],"uid":42,"active":true,"future":23}],
        "georef":{"datum":{"lat":-15.,"lon":-72.,"alt":100.,"north":12.,"future":29}}
    });
    write_plain(&doc,&src); let mut scene=load_igz(&src).unwrap();
    assert_eq!(scene.saved_views[0].fov_deg,62.);
    assert!(!scene.saved_views[0].perspective);
    assert_eq!(scene.section_planes[0].id.as_u128(),42);
    assert!(scene.geo_datum.as_ref().unwrap().east0.abs()>0.);
    scene.saved_views[0].fov_deg=70.; scene.saved_views[0].perspective=true; scene.saved_views[0].parallel=false;
    scene.section_planes[0].point.x=9.;
    scene.geo_datum=Some(ingetrazo_core::georef::SceneDatum::new(-16.,-73.,200.,13.));
    save_igz(&scene,&dst).unwrap();
    let out=scene_to_igz_document(&scene);
    assert_eq!(out["saved_views"][0]["fov"],70.);
    assert_eq!(out["saved_views"][0]["parallel"],false);
    assert_eq!(out["saved_views"][0]["style"],doc["saved_views"][0]["style"]);
    assert_eq!(out["section_planes"][0]["uid"],42);
    assert_eq!(out["section_planes"][0]["future"],23);
    assert_eq!(out["georef"]["datum"]["future"],29);
    let reopened=load_igz(&dst).unwrap();
    assert_eq!(reopened.saved_views[0].fov_deg,70.);
    assert_eq!(reopened.section_planes[0].point.x,9.);
    assert_eq!(reopened.geo_datum.unwrap().lat,-16.);
    std::fs::remove_file(src).unwrap(); std::fs::remove_file(dst).unwrap();
}

#[test]
fn original_composer_edits_preserve_unsupported_items_and_title_rows() {
    use ingetrazo_core::composition::{SheetItem,PaperSize,Orientation};
    let src=temp("composer-source.igz"); let dst=temp("composer-save.igz");
    let doc=json!({"edges":[],"faces":[],"compositions":[{
        "name":"original","paper":"A4","landscape":true,"margin_mm":10.,"opaque":71,
        "frames":[{"x_mm":15.,"y_mm":16.,"w_mm":80.,"h_mm":90.,"scale_n":40.,"view_key":"std:top","uid":"0123456789abcdef0123456789abcdef","paper_bg":true,"unknown":72}],
        "texts":[{"x_mm":20.,"text":"original","italic":true,"unknown":73}],
        "images":[{"path":"retain-image","unknown":74}],
        "cajetin":{"proyecto":"legacy","campos":[["PROYECTO","project"],["CUSTOM","keep"]],"line_color":"#abc"}
    }]});
    write_plain(&doc,&src);let mut scene=load_igz(&src).unwrap();
    assert_eq!(scene.sheets.len(),1);
    let sheet=&mut scene.sheets[0];assert_eq!(sheet.items.len(),3);
    sheet.name="renamed".into();sheet.paper_size=PaperSize::A3;sheet.orientation=Orientation::Portrait;
    for item in &mut sheet.items {
        match item {
            SheetItem::ViewFrame(frame)=>{frame.rect.x=25.;frame.scale_n=50.;},
            SheetItem::Text(text)=>{text.text="changed".into();},
            SheetItem::TitleBlock(title)=>{assert_eq!(title.project_name,"project");title.project_name="new project".into();},
            _=>{}
        }
    }
    save_igz(&scene,&dst).unwrap();let out=scene_to_igz_document(&scene);
    let sheet=&out["compositions"][0];
    assert_eq!(sheet["name"],"renamed");assert_eq!(sheet["paper"],"A3");assert_eq!(sheet["landscape"],false);
    assert_eq!(sheet["opaque"],71);assert_eq!(sheet["frames"][0]["unknown"],72);
    assert_eq!(sheet["frames"][0]["uid"],doc["compositions"][0]["frames"][0]["uid"]);
    assert_eq!(sheet["texts"][0]["unknown"],73);assert_eq!(sheet["texts"][0]["text"],"changed");
    assert_eq!(sheet["images"],doc["compositions"][0]["images"]);
    assert_eq!(sheet["cajetin"]["campos"][0][1],"new project");assert_eq!(sheet["cajetin"]["campos"][1][1],"keep");
    let reopened=load_igz(&dst).unwrap();assert_eq!(reopened.sheets[0].items.len(),3);
    scene.sheets[0].items.retain(|v|!matches!(v,SheetItem::Text(_)));
    assert_eq!(scene_to_igz_document(&scene)["compositions"][0]["texts"].as_array().unwrap().len(),0);
    std::fs::remove_file(src).unwrap();std::fs::remove_file(dst).unwrap();
}

#[test]
fn replacement_face_does_not_inherit_deleted_face_metadata() {
    let src=temp("replace-source.igz");
    let doc=json!({"edges":[],"faces":[{"vertices":[[0,0,0],[1,0,0],[0,1,0]],"opaque_deleted":99}]});
    write_plain(&doc,&src); let mut scene=load_igz(&src).unwrap();
    scene.mesh.faces.clear();
    let ids=[DVec3::new(0.,0.,5.),DVec3::new(1.,0.,5.),DVec3::new(0.,1.,5.)].map(|p|scene.mesh.add_vertex(p));
    scene.mesh.add_face(ids.to_vec(),vec![]).unwrap();
    assert!(scene_to_igz_document(&scene)["faces"][0].get("opaque_deleted").is_none());
    std::fs::remove_file(src).unwrap();
}


#[test]
fn survey_points_and_paths_preserve_metadata_through_rename_reorder_and_edit() {
    let src=temp("geographic-source.igz"); let dst=temp("geographic-save.igz");
    let doc=json!({"edges":[],"faces":[],
        "geo_points":[{"position":[1.,2.,3.],"name":"P1","desc":"survey","opaque":1},{"position":[4.,5.,6.],"name":"P2","opaque":2}],
        "geo_paths":[{"points":[[0.,0.,0.],[1.,0.,0.],[1.,1.,0.]],"name":"plot","closed":true,"surface":"draped","opaque":3}]});
    write_plain(&doc,&src); let mut scene=load_igz(&src).unwrap();
    assert_eq!(scene.geo_points.len(),2); assert!(scene.geo_paths[0].surface.is_some());
    scene.geo_points.swap(0,1);scene.geo_points[1].name="renamed".into();scene.geo_points[1].position.z=9.;
    scene.geo_paths[0].points[1].z=5.;scene.geo_paths[0].name="changed".into();
    save_igz(&scene,&dst).unwrap(); let out=scene_to_igz_document(&scene);
    assert_eq!(out["geo_points"][0]["opaque"],2);assert_eq!(out["geo_points"][1]["opaque"],1);
    assert_eq!(out["geo_paths"][0]["opaque"],3);assert_eq!(out["geo_paths"][0]["surface"],"draped");
    let reopened=load_igz(&dst).unwrap();
    assert_eq!(reopened.geo_points[1].position.z,9.);assert_eq!(reopened.geo_paths[0].points[1].z,5.);
    std::fs::remove_file(src).unwrap();std::fs::remove_file(dst).unwrap();
}


#[test]
fn fresh_rust_sheet_saves_in_python_composition_schema() {
    use ingetrazo_core::{scene::Scene,composition::Sheet};
    let dst=temp("fresh-sheet.igz");let mut scene=Scene::new();scene.sheets.push(Sheet::default());
    save_igz(&scene,&dst).unwrap();let out=scene_to_igz_document(&scene);
    assert_eq!(out["compositions"][0]["paper"],"A3");
    assert!(out["compositions"][0]["frames"][0].get("x_mm").is_some());
    assert_eq!(out["compositions"][0]["frames"][0]["style"],"vectorial");
    assert!(out["compositions"][0]["cajetin"].get("proyecto").is_some());
    assert_eq!(load_igz(&dst).unwrap().sheets[0].items.len(),2);
    std::fs::remove_file(dst).unwrap();
}


#[test]
fn renamed_and_reordered_views_and_sections_keep_identity_metadata() {
    let src=temp("view-identity-source.igz");let dst=temp("view-identity-save.igz");
    let doc=json!({"edges":[],"faces":[],
        "saved_views":[{"name":"A","style":{"unknown":1}},{"name":"B","style":{"unknown":2}}],
        "section_planes":[{"uid":1,"point":[0.,0.,0.],"normal":[0.,0.,1.],"name":"A","unknown":3},{"uid":2,"point":[1.,0.,0.],"normal":[0.,1.,0.],"name":"B","unknown":4}]});
    write_plain(&doc,&src);let mut scene=load_igz(&src).unwrap();
    scene.saved_views.swap(0,1);scene.saved_views[1].name="renamed".into();
    scene.section_planes.swap(0,1);scene.section_planes[1].name="renamed".into();
    save_igz(&scene,&dst).unwrap();let out=scene_to_igz_document(&scene);
    assert_eq!(out["saved_views"][0]["style"]["unknown"],2);
    assert_eq!(out["saved_views"][1]["style"]["unknown"],1);
    assert_eq!(out["section_planes"][0]["unknown"],4);
    assert_eq!(out["section_planes"][1]["unknown"],3);
    std::fs::remove_file(src).unwrap();std::fs::remove_file(dst).unwrap();
}


#[test]
fn explicit_components_share_updates_while_plain_proto_groups_remain_independent() {
    let src=temp("component-source.igz");let dst=temp("component-save.igz");
    let identity=json!([1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.]);
    let doc=json!({"edges":[],"faces":[],"protos":[{"edges":[],"faces":[{"vertices":[[0.,0.,0.],[1.,0.,0.],[0.,1.,0.]],"opaque":9}]}],
        "groups":[{"name":"linked1","proto":0,"component":true,"xform":identity},{"name":"linked2","proto":0,"component":true,"xform":identity},
        {"name":"plain","proto":0,"component":false,"xform":identity}]});
    write_plain(&doc,&src);let mut scene=load_igz(&src).unwrap();
    let id=scene.groups[0].component_definition.unwrap();
    assert_eq!(scene.groups[1].component_definition,Some(id));assert_eq!(scene.groups[2].component_definition,None);
    let mut def=scene.component_definitions[&id].clone();def.mesh.vertices[0].position.z=7.;
    scene.replace_component_definition(id,def).unwrap();
    assert_eq!(scene.groups[0].mesh.vertices[0].position.z,7.);assert_eq!(scene.groups[1].mesh.vertices[0].position.z,7.);
    assert_eq!(scene.groups[2].mesh.vertices[0].position.z,0.);
    save_igz(&scene,&dst).unwrap();let out=scene_to_igz_document(&scene);
    assert_eq!(out["groups"][0]["proto"],out["groups"][1]["proto"]);assert_ne!(out["groups"][0]["proto"],out["groups"][2]["proto"]);
    let reopened=load_igz(&dst).unwrap();assert_eq!(reopened.groups[0].component_definition,reopened.groups[1].component_definition);
    scene.make_component_unique(scene.groups[1].id).unwrap();assert_eq!(scene.groups[1].component_definition,None);
    save_igz(&scene,&dst).unwrap();let reopened=load_igz(&dst).unwrap();
    assert_eq!(reopened.groups[1].component_definition,None);
    std::fs::remove_file(src).unwrap();std::fs::remove_file(dst).unwrap();
}


#[test]
fn texture_files_with_same_basename_get_distinct_archive_entries() {
    use ingetrazo_core::{scene::Scene,materials::Material};
    let dir=temp("textures");std::fs::create_dir_all(dir.join("a")).unwrap();std::fs::create_dir_all(dir.join("b")).unwrap();
    let a=dir.join("a/paint.png");let b=dir.join("b/paint.png");
    std::fs::write(&a,[1,2,3]).unwrap();std::fs::write(&b,[4,5,6]).unwrap();
    let mut scene=Scene::new();scene.materials.insert("a".into(),Material::with_texture("a",a.to_string_lossy()));
    scene.materials.insert("b".into(),Material::with_texture("b",b.to_string_lossy()));
    let dst=dir.join("saved.igz");save_igz(&scene,&dst).unwrap();let reopened=load_igz(&dst).unwrap();
    let ea=reopened.materials["a"].texture_embed.as_ref().unwrap();let eb=reopened.materials["b"].texture_embed.as_ref().unwrap();
    assert_ne!(ea,eb);assert_eq!(reopened.embedded_textures[ea],vec![1,2,3]);assert_eq!(reopened.embedded_textures[eb],vec![4,5,6]);
    let plain=dir.join("plain.igz");std::fs::write(&plain,b"original").unwrap();
    assert!(ingetrazo_formats::igz::save_igz_format1(&reopened,&plain).is_err());assert_eq!(std::fs::read(&plain).unwrap(),b"original");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn dimensions_use_original_schema_and_keep_identity_on_reorder_edits() {
    let source=temp("dimension-source.igz");let target=temp("dimension-result.igz");
    let original=json!({"edges":[],"faces":[],"dimensions":[{"a":[0.,0.,0.],"b":[3.,4.,0.],"offset":[-2.,0.,0.],"text":"width <> cm","axis":"x","layer":"Annotations","opaque":{"id":1}},{"a":[1.,2.,3.],"b":[4.,6.,3.],"offset":[0.,1.,0.],"opaque":{"id":2}}]});
    write_plain(&original,&source);let mut scene=load_igz(&source).unwrap();assert_eq!(scene.dimensions[0].length(),3.);assert!(scene.dimensions[0].display_text(ingetrazo_core::units::LengthUnit::Meters,2).starts_with("width 3"));
    scene.dimensions.reverse();scene.dimensions[1].end.x=7.;scene.dimensions[1].text_override=Some("edited <>".into());save_igz(&scene,&target).unwrap();
    let reopened=load_igz(&target).unwrap();let doc=scene_to_igz_document(&reopened);assert_eq!(doc["dimensions"][0]["opaque"]["id"],2);assert_eq!(doc["dimensions"][1]["opaque"]["id"],1);assert_eq!(doc["dimensions"][1]["b"][0],7.);assert_eq!(doc["dimensions"][1]["text"],"edited <>");assert!(doc["dimensions"][1].get("start").is_none());
    std::fs::remove_file(source).unwrap();std::fs::remove_file(target).unwrap();
}

#[test]
fn fresh_saved_view_references_exact_persisted_group_and_section_uids() {
    use ingetrazo_core::{scene::Scene,groups::Group,section::SectionPlane,saved_views::SavedView};
    let mut scene=Scene::new();let mut parent=Group::new("parent");let mut child=Group::new("hidden child");child.hidden=true;parent.children.push(child);scene.groups.push(parent);
    scene.section_planes.push(SectionPlane::new(DVec3::ZERO,DVec3::Z,"cut","A"));scene.saved_views.push(SavedView::capture_scene("view",&scene,&scene.camera));
    let document=scene_to_igz_document(&scene);assert_eq!(document["saved_views"][0]["hidden_objects"][0],document["groups"][0]["children"][0]["uid"]);assert_eq!(document["saved_views"][0]["section"]["active"],document["section_planes"][0]["uid"]);
    let path=temp("view-identities.igz");save_igz(&scene,&path).unwrap();let mut restored=load_igz(&path).unwrap();restored.groups[0].children[0].hidden=false;restored.section_planes[0].active=false;let view=restored.saved_views[0].clone();let mut camera=restored.camera.clone();view.apply_scene(&mut restored,&mut camera);assert!(restored.groups[0].children[0].hidden);assert!(restored.section_planes[0].active);
    let original_uid=restored.groups[0].native_uid();let duplicate=restored.groups[0].duplicate_instance();assert_ne!(duplicate.native_uid(),original_uid);std::fs::remove_file(path).unwrap();
}

#[test]fn frame_camera_native_edit_preserves_opaque_metadata_and_reopens_lens(){
 use ingetrazo_core::composition::SheetItem;
 let src=temp("frame-camera-source.igz");let dst=temp("frame-camera-edit.igz");let doc=json!({"edges":[],"faces":[],"compositions":[{"name":"Frame","paper":"A3","frames":[{"uid":"0123456789abcdef0123456789abcdef","cam_target":[1.,2.,3.],"cam_yaw":0.2,"cam_pitch":0.4,"cam_distance":9.,"cam_fov":55.,"perspective":true,"vendor_lens":{"uninterpreted":7}}]}]});write_plain(&doc,&src);let mut scene=load_igz(&src).unwrap();let SheetItem::ViewFrame(frame)=&mut scene.sheets[0].items[0]else{panic!()};assert_eq!(frame.cam_target,Some([1.,2.,3.]));assert_eq!(frame.cam_distance,Some(9.));frame.cam_target=Some([4.,5.,6.]);frame.cam_distance=Some(12.);frame.cam_fov=Some(70.);save_igz(&scene,&dst).unwrap();let raw=scene_to_igz_document(&scene);assert_eq!(raw["compositions"][0]["frames"][0]["vendor_lens"],doc["compositions"][0]["frames"][0]["vendor_lens"]);let reopened=load_igz(&dst).unwrap();let SheetItem::ViewFrame(frame)=&reopened.sheets[0].items[0]else{panic!()};assert_eq!(frame.cam_target,Some([4.,5.,6.]));assert_eq!(frame.cam_distance,Some(12.));assert_eq!(frame.cam_fov,Some(70.));assert!(frame.perspective);std::fs::remove_file(src).unwrap();std::fs::remove_file(dst).unwrap();
}

#[test]
fn edited_vector_ink_controls_reopen_with_original_field_names() {
 use ingetrazo_core::composition::SheetItem;
 let src=temp("ink-source.igz");let dst=temp("ink-edit.igz");
 let doc=json!({"edges":[],"faces":[],"compositions":[{"name":"Ink","paper":"A3","frames":[{"uid":"0123456789abcdef0123456789abcdef","pen_cut_mm":0.7,"pen_profile_mm":0.4,"pen_edge_mm":0.2,"profiles":false,"hidden_lines":true,"vendor_ink":{"value":7}}]}]});
 write_plain(&doc,&src);let mut scene=load_igz(&src).unwrap();
 let SheetItem::ViewFrame(frame)=&mut scene.sheets[0].items[0]else{panic!()};
 assert_eq!(frame.pen_cut_mm,0.7);assert!(!frame.profiles);assert!(frame.hidden_lines);
 frame.pen_cut_mm=0.9;frame.pen_profile_mm=0.6;frame.pen_edge_mm=0.15;frame.profiles=true;frame.hidden_lines=false;frame.paper_bg=true;frame.cut_fill="hatch".into();frame.cut_fill_color="#123456".into();frame.cut_hatch_mm=2.3;
 save_igz(&scene,&dst).unwrap();let raw=scene_to_igz_document(&scene);
 assert_eq!(raw["compositions"][0]["frames"][0]["vendor_ink"],json!({"value":7}));
 let reopened=load_igz(&dst).unwrap();let SheetItem::ViewFrame(frame)=&reopened.sheets[0].items[0]else{panic!()};
 assert_eq!(frame.pen_cut_mm,0.9);assert_eq!(frame.pen_profile_mm,0.6);assert_eq!(frame.pen_edge_mm,0.15);assert!(frame.profiles);assert!(!frame.hidden_lines);assert!(frame.paper_bg);assert_eq!(frame.cut_fill,"hatch");assert_eq!(frame.cut_fill_color,"#123456");assert_eq!(frame.cut_hatch_mm,2.3);
 std::fs::remove_file(src).unwrap();std::fs::remove_file(dst).unwrap();
}
