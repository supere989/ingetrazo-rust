use glam::DVec3;
use ingetrazo_core::{drafting, mesh::Mesh};
#[test]
fn source_cut_order_is_existing_edge_order_not_coordinate_order() {
    let mut m = Mesh::new();
    for x in [0.5, -0.5] {
        let a = m.add_vertex(DVec3::new(x, -1., 0.));
        let b = m.add_vertex(DVec3::new(x, 1., 0.));
        m.add_edge(a, b).unwrap();
    }
    let a = m.add_vertex(DVec3::new(5., 0., 1.0000000000000002));
    let b = m.add_vertex(DVec3::new(6., 0., 1.0000000000000002));
    let e = m.add_edge(a, b).unwrap();
    m.edges[e].hidden = true;
    m.edges[e].soft = true;
    m.edges[e].attrs.insert("vendor".into(), 71.into());
    let vertex_ids = m.vertices.iter().map(|v| v.uid).collect::<Vec<_>>();
    let old_edge = m.edges[e].clone();
    let out = drafting::add_segments(
        &m,
        &[(DVec3::new(-1., 0., 0.), DVec3::new(1., 0., 0.))],
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        out.vertices
            .iter()
            .take(m.vertices.len())
            .map(|v| v.position)
            .collect::<Vec<_>>(),
        m.vertices.iter().map(|v| v.position).collect::<Vec<_>>()
    );
    assert_eq!(
        out.vertices
            .iter()
            .take(m.vertices.len())
            .map(|v| v.uid)
            .collect::<Vec<_>>(),
        vertex_ids
    );
    assert_eq!(
        out.vertices[m.vertices.len()..]
            .iter()
            .map(|v| v.position)
            .collect::<Vec<_>>(),
        vec![
            DVec3::new(0.5, 0., 0.),
            DVec3::new(-0.5, 0., 0.),
            DVec3::new(-1., 0., 0.),
            DVec3::new(1., 0., 0.)
        ]
    );
    assert_eq!(out.edges[0].history_id, old_edge.history_id);
    assert_eq!(out.edges[0].attrs, old_edge.attrs);
    assert!(out.edges[0].hidden && out.edges[0].soft);
    assert!(out.edges[1..]
        .iter()
        .all(|e| e.history_id != m.edges[0].history_id && e.history_id != m.edges[1].history_id));
    assert_eq!(
        out.edges.iter().map(|e| [e.v0, e.v1]).collect::<Vec<_>>(),
        vec![
            [4, 5],
            [0, 6],
            [6, 1],
            [2, 7],
            [7, 3],
            [8, 7],
            [7, 6],
            [6, 9]
        ]
    );
}
#[test]
fn scoped_plane_faces_are_fresh_but_reuse_existing_edge_objects() {
    let mut m = Mesh::new();
    let ids = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| m.add_vertex(p));
    let f = m.add_face(ids.to_vec(), vec![]).unwrap();
    let old_face = m.faces[f].history_id;
    let old_edges = m.edges.iter().map(|e| e.history_id).collect::<Vec<_>>();
    let out = drafting::add_segments(
        &m,
        &[(DVec3::new(2., 0., 0.), DVec3::new(3., 0., 0.))],
        None,
        None,
    )
    .unwrap();
    assert_eq!(out.faces[0].vertices, ids);
    assert_ne!(out.faces[0].history_id, old_face);
    assert_eq!(
        out.edges
            .iter()
            .take(3)
            .map(|e| e.history_id)
            .collect::<Vec<_>>(),
        old_edges
    );
    assert_eq!(
        out.vertices
            .iter()
            .take(3)
            .map(|v| v.uid)
            .collect::<Vec<_>>(),
        m.vertices.iter().map(|v| v.uid).collect::<Vec<_>>()
    );
}

#[test]
fn scoped_graph_start_ignores_an_earlier_off_plane_edge() {
    let mut m = Mesh::new();
    let b = m.add_vertex(DVec3::X);
    let off = m.add_vertex(DVec3::X + DVec3::Z);
    m.add_edge(b, off).unwrap();
    let a = m.add_vertex(DVec3::ZERO);
    let c = m.add_vertex(DVec3::Y);
    m.add_face(vec![a, b, c], vec![]).unwrap();
    let out = drafting::add_segments(
        &m,
        &[(DVec3::new(2., 0., 0.), DVec3::new(3., 0., 0.))],
        None,
        None,
    )
    .unwrap();
    assert_eq!(out.faces[0].vertices, vec![a, b, c]);
    assert_eq!(out.edges[0].history_id, m.edges[0].history_id);
}

#[test]
fn split_constructor_defaults_do_not_overwrite_welded_or_opaque_objects() {
    let mut m = Mesh::new();
    let a = m.add_vertex(DVec3::new(0., -1., 0.));
    let b = m.add_vertex(DVec3::new(0., 1., 0.));
    let p = m.add_vertex(DVec3::ZERO);
    let long = m.add_edge(a, b).unwrap();
    m.edges[long].soft = true;
    m.edges[long].hidden = true;
    m.edges[long].layer = Some("Old".into());
    m.edges[long].attrs.insert("vendor".into(), 71.into());
    let weld = m.add_edge(a, p).unwrap();
    m.edges[weld].hidden = true;
    m.edges[weld].layer = Some("Weld".into());
    m.edges[weld].attrs.insert("vendor".into(), 72.into());
    let retained = m.edges[weld].clone();
    let out = drafting::add_segments(
        &m,
        &[(DVec3::new(-1., 0., 0.), DVec3::new(1., 0., 0.))],
        None,
        None,
    )
    .unwrap();
    let welded = out
        .edges
        .iter()
        .find(|e| e.history_id == retained.history_id)
        .unwrap();
    assert_eq!(welded.attrs, retained.attrs);
    assert_eq!(welded.hidden, retained.hidden);
    assert_eq!(welded.soft, retained.soft);
    assert_eq!(welded.layer, retained.layer);
    let upper = out
        .edges
        .iter()
        .find(|e| (e.v0 == p && e.v1 == b) || (e.v0 == b && e.v1 == p))
        .unwrap();
    assert!(upper.soft);
    assert!(!upper.hidden);
    assert_eq!(upper.layer, None);
    assert!(upper.attrs.is_empty());
    assert_ne!(upper.history_id, m.edges[long].history_id);
}
