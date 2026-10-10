use glam::DVec3;
use ingetrazo_core::scene::{Scene, SelectionItem};
use ingetrazo_formats::skp::{self, SkpError};
const REAL: &[u8] = include_bytes!("../../../docs/audits/skp-native-vff/SU_File.skp");
const TEXTURE: &[u8] =
    include_bytes!("../../../docs/audits/skp-native-vff/controls/texture-refusal.skp");
#[test]
fn actual_vff_geometry_and_shared_leaf_instances() {
    let p = skp::parse_skp_bytes(REAL, "SU_File").unwrap();
    let scene = skp::scene_from_payload(&p).unwrap();
    let mesh = &scene.groups[0].mesh;
    assert_eq!(
        (mesh.vertices.len(), mesh.edges.len(), mesh.faces.len()),
        (56, 84, 32)
    );
    let shared = include_bytes!("../../../docs/audits/skp-native-vff/controls/shared-eight.skp");
    let scene =
        skp::scene_from_payload(&skp::parse_skp_bytes(shared, "shared-eight").unwrap()).unwrap();
    assert_eq!(scene.groups.len(), 8);
    assert_eq!(scene.mesh_shares.len(), 1);
    let id = scene.groups[0].mesh_share.unwrap();
    assert!(scene.groups.iter().all(|g| g.mesh_share == Some(id)));
    assert_eq!(scene.groups[1].transform.w_axis.x, 50.8f64 as f32 as f64);
}
#[test]
fn unsupported_and_malformed_import_preserve_destination() {
    let mut scene = Scene::new();
    scene.mesh.add_vertex(DVec3::new(2., 3., 4.));
    scene.select(SelectionItem::Vertex(0));
    let before = scene.clone();
    let path =
        std::env::temp_dir().join(format!("ingetrazo-skp-atomic-{}.skp", uuid::Uuid::new_v4()));
    for data in [TEXTURE, &REAL[..REAL.len() / 2], b"not a skp".as_slice()] {
        std::fs::write(&path, data).unwrap();
        assert!(skp::replace_from_skp(&mut scene, &path).is_err());
        assert_eq!(scene, before);
    }
    std::fs::remove_file(path).unwrap();
    assert!(matches!(
        skp::parse_skp_bytes(&REAL[..4], "legacy"),
        Err(SkpError::Unsupported(_))
    ));
}
#[test]
fn invalid_neutral_payload_cannot_publish_partial_geometry() {
    let mut p = skp::parse_skp_bytes(REAL, "SU_File").unwrap();
    let id = *p.root.vertices.keys().next().unwrap();
    p.root.vertices.get_mut(&id).unwrap()[0] = f64::NAN;
    assert!(skp::scene_from_payload(&p).is_err());
    let mut p = skp::parse_skp_bytes(REAL, "SU_File").unwrap();
    let f = p.root.faces.values_mut().next().unwrap();
    f.loops[0].clear();
    assert!(skp::scene_from_payload(&p).is_err());
    let mut p = skp::parse_skp_bytes(REAL, "SU_File").unwrap();
    p.root.unsupported.insert("required annotation".into());
    assert!(matches!(
        skp::scene_from_payload(&p),
        Err(SkpError::Unsupported(_))
    ));
}

#[test]
fn nested_prototypes_tags_and_equal_materials_retain_source_structure() {
    let parse = |bytes: &[u8]| {
        skp::scene_from_payload(&skp::parse_skp_bytes(bytes, "nested").unwrap()).unwrap()
    };
    let scene = parse(include_bytes!(
        "../../../docs/audits/skp-native-vff/nested-controls/nested-proto-in-proto.skp"
    ));
    assert_eq!(scene.groups.len(), 8);
    assert_eq!(scene.mesh_shares.len(), 2);
    let parent = scene.groups[0].mesh_share;
    let child = scene.groups[0].children[0].mesh_share;
    assert!(scene.groups.iter().all(|g| g.mesh_share == parent
        && g.children.len() == 1
        && g.children[0].mesh_share == child));
    assert_eq!(scene.groups[0].mesh.faces.len(), 1);
    assert_eq!(scene.groups[0].children[0].mesh.faces.len(), 64);
    let scene = parse(include_bytes!(
        "../../../docs/audits/skp-native-vff/nested-controls/nested-tagged-shared.skp"
    ));
    assert_eq!(scene.groups.len(), 18);
    assert_eq!(
        scene
            .groups
            .iter()
            .filter(|g| g.layer.as_deref() == Some("Hidden tag"))
            .count(),
        16
    );
    let scene = parse(include_bytes!(
        "../../../docs/audits/skp-native-vff/nested-controls/nested-equal-paint-protos.skp"
    ));
    assert_eq!(scene.mesh_shares.len(), 1);
}
#[test]
fn malformed_reachable_graph_and_unsupported_billboard_refuse_privately() {
    let bytes = include_bytes!(
        "../../../docs/audits/skp-native-vff/nested-controls/nested-single-container.skp"
    );
    let p = skp::parse_skp_bytes(bytes, "nested").unwrap();
    let mut cycle = p.clone();
    cycle.definitions.get_mut(&6000).unwrap().instances[0].definition = 6000;
    assert!(matches!(
        skp::scene_from_payload(&cycle),
        Err(SkpError::Invalid(_))
    ));
    let mut missing = p.clone();
    missing.definitions.remove(&5000);
    assert!(matches!(
        skp::scene_from_payload(&missing),
        Err(SkpError::Invalid(_))
    ));
    let mut overflow = p.clone();
    overflow.root.instances[0].matrix[0] = 1e38;
    overflow
        .definitions
        .get_mut(&5000)
        .unwrap()
        .vertices
        .values_mut()
        .for_each(|v| v[0] = 1e38);
    assert!(matches!(
        skp::scene_from_payload(&overflow),
        Err(SkpError::Invalid(_))
    ));
    let mut billboard = p.clone();
    billboard.definitions.get_mut(&5000).unwrap().billboard = true;
    assert!(matches!(
        skp::scene_from_payload(&billboard),
        Err(SkpError::Unsupported(_))
    ));
}
