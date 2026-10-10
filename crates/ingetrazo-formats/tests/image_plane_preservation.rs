use ingetrazo_formats::igz::{load_igz, save_igz};
use serde_json::json;
#[test]
fn removing_first_plane_keeps_survivor_opaque_fields_and_resets_optional_defaults() {
    let path = std::env::temp_dir().join(format!(
        "ingetrazo-image-plane-{}.igz",
        uuid::Uuid::new_v4()
    ));
    let data = json!({"igz_format":1,"app_version":"fixture","scene":{"image_planes":[{"texture":{"embed":"textures/scan.png"},"origin":[0,0,0],"u":[1,0,0],"v":[0,1,0],"aspect":1,"name":"First","vendor":{"token":"first"}},{"texture":{"embed":"textures/scan.png","vendor":19},"origin":[3,4,5],"u":[2,0,0],"v":[0,1,0],"aspect":0.5,"name":"Second","opacity":0.5,"locked":true,"layer":"Tagged","vendor":{"token":"second"}}]}});
    std::fs::write(&path, serde_json::to_vec(&data).unwrap()).unwrap();
    let mut scene = load_igz(&path).unwrap();
    scene
        .embedded_textures
        .insert("textures/scan.png".into(), vec![1, 2, 3]);
    scene.image_planes.remove(0);
    let p = &mut scene.image_planes[0];
    p.name.clear();
    p.opacity = 1.;
    p.locked = false;
    p.layer = Some("Images".into());
    save_igz(&scene, &path).unwrap();
    let read = load_igz(&path).unwrap();
    assert_eq!(read.image_planes.len(), 1);
    let p = &read.image_planes[0];
    assert_eq!(p.name, "");
    assert_eq!(p.opacity, 1.);
    assert!(!p.locked);
    assert_eq!(p.layer.as_deref(), Some("Images"));
    assert_eq!(p.native_document["vendor"]["token"], "second");
    assert_eq!(p.native_document["texture"]["vendor"], 19);
    assert_eq!(read.embedded_textures["textures/scan.png"], vec![1, 2, 3]);
    std::fs::remove_file(path).unwrap();
}
