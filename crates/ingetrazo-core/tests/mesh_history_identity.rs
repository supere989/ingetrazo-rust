use ingetrazo_core::mesh::{Mesh, MeshEdge, MeshFace};
#[test]
fn row_tokens_are_clone_owned_and_not_persisted_or_semantic() {
    let edge = MeshEdge::new(0, 1);
    let face = MeshFace::new(vec![0, 1, 2]);
    assert_eq!(edge.history_id, edge.clone().history_id);
    assert_eq!(face.history_id, face.clone().history_id);
    let edge_json = serde_json::to_value(&edge).unwrap();
    let face_json = serde_json::to_value(&face).unwrap();
    assert!(edge_json.get("history_id").is_none());
    assert!(face_json.get("history_id").is_none());
    let decoded_edge: MeshEdge = serde_json::from_value(edge_json).unwrap();
    let decoded_face: MeshFace = serde_json::from_value(face_json).unwrap();
    assert_eq!(edge, decoded_edge);
    assert_eq!(face, decoded_face);
    assert_ne!(edge.history_id, decoded_edge.history_id);
    assert_ne!(face.history_id, decoded_face.history_id);
    assert_ne!(edge.history_id, MeshEdge::new(0, 1).history_id);
    assert_ne!(face.history_id, MeshFace::new(vec![0, 1, 2]).history_id);
}
#[test]
fn compaction_keeps_object_identity_and_recreation_is_fresh() {
    let mut m = Mesh::new();
    let a = m.add_vertex(glam::DVec3::ZERO);
    let b = m.add_vertex(glam::DVec3::X);
    let c = m.add_vertex(glam::DVec3::Y);
    m.add_edge(a, b);
    m.add_edge(a, c);
    let removed = m.edges[0].history_id;
    let kept = m.edges[1].history_id;
    let snapshot = m.clone();
    m.edges.remove(0);
    m.rebuild_incidence();
    assert_eq!(m.edges[0].history_id, kept);
    m.add_edge(a, b);
    assert_ne!(m.edges[1].history_id, removed);
    assert_eq!(snapshot.edges[0].history_id, removed);
    let mut decoded: Mesh =
        serde_json::from_value(serde_json::to_value(&snapshot).unwrap()).unwrap();
    // Serde intentionally omits the existing derived spatial cache.
    decoded.rebuild_spatial_index();
    assert_eq!(snapshot, decoded);
}

#[test]
fn face_compaction_rebuilds_radial_indices_without_recreating_rows() {
    let mut m = Mesh::new();
    let v = [
        glam::DVec3::ZERO,
        glam::DVec3::X,
        glam::DVec3::Y,
        glam::DVec3::NEG_Y,
    ]
    .map(|p| m.add_vertex(p));
    m.add_face(vec![v[0], v[1], v[2]], vec![]);
    m.add_face(vec![v[1], v[0], v[3]], vec![]);
    let edge = m.find_edge(v[0], v[1]).unwrap();
    let edge_identity = m.edges[edge].history_id;
    let face_identity = m.faces[1].history_id;
    assert_eq!(m.edges[edge].faces, vec![0, 1]);
    m.faces.remove(0);
    m.rebuild_incidence();
    assert_eq!(m.edges[edge].faces, vec![0]);
    assert_eq!(m.edges[edge].history_id, edge_identity);
    assert_eq!(m.faces[0].history_id, face_identity);
    // Partition collapse can retain a boundary also present in a hole;
    // radial incidence retains both boundary occurrences without changing that face object.
    let boundary = m.faces[0].vertices.clone();
    m.faces[0].holes.push(boundary);
    m.rebuild_incidence();
    assert_eq!(m.edges[edge].faces, vec![0, 0]);
    assert_eq!(m.faces[0].history_id, face_identity);
}
