use glam::DVec3;
use ingetrazo_render::process_hlr::{worker_main, HlrProcessPool};
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|s| s == "--hlr-worker") {
        worker_main(std::path::Path::new(&args[2])).unwrap();
        return;
    }
    let executable = std::env::current_exe().unwrap();
    let mut pool = HlrProcessPool::new(&executable, 2, 3, 4096).unwrap();
    let edges = vec![(DVec3::new(-2., 0., 2.), DVec3::new(2., 0., 2.)); 13];
    let triangles = vec![[
        DVec3::new(-1., -1., 0.),
        DVec3::new(1., -1., 0.),
        DVec3::new(0., 1., 0.),
    ]];
    let pids = pool.process_ids();
    assert_eq!(pids.len(), 2);
    assert_ne!(pids[0], pids[1]);
    for _ in 0..3 {
        let result = pool.intervals(&edges, &triangles, 1e-9).unwrap();
        assert_eq!(pool.process_ids(), pids);
        assert_eq!(result.len(), 13);
        assert!(result.iter().all(|row| row.len() == 1));
        assert!((result[0][0].0 - 0.375).abs() < 1e-9);
        assert!((result[0][0].1 - 0.625).abs() < 1e-9);
        println!("{:?}", pool.metrics);
    }
    let mut scene = ingetrazo_core::scene::Scene::new();
    for z in [0., 2.] {
        let ids = [(-2., -2.), (2., -2.), (2., 2.), (-2., 2.)]
            .map(|(x, y)| scene.mesh.add_vertex(DVec3::new(x, y, z)));
        scene.mesh.add_face(ids.to_vec(), vec![]);
    }
    let mut camera = ingetrazo_core::camera::OrbitCamera::default();
    for angle in [0., 0.4, 1.2, 2.] {
        camera.yaw = angle;
        for profiles in [false, true] {
            for hidden in [false, true] {
                let serial = ingetrazo_render::vector_hlr::parallel_segments(
                    &scene, &camera, profiles, hidden,
                );
                let processes = ingetrazo_render::vector_hlr::parallel_segments_with_pool(
                    &scene, &camera, profiles, hidden, &mut pool,
                )
                .unwrap();
                assert_eq!(serial.len(), processes.len());
                for (a, b) in serial.iter().zip(&processes) {
                    assert_eq!(a.a, b.a);
                    assert_eq!(a.b, b.b);
                    assert_eq!(a.kind, b.kind);
                }
            }
        }
    }
    assert!(pool
        .intervals(&[(DVec3::NAN, DVec3::ZERO)], &triangles, 1e-9)
        .is_err());
    let mut bounded = HlrProcessPool::new(&executable, 1, 3, 24).unwrap();
    assert!(bounded.intervals(&edges, &triangles, 1e-9).is_err());
    println!("persistent two-process HLR, repeated revisions, ordering, nonfinite and capacity controls PASS");
}
