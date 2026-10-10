use glam::DVec3 as V;
use ingetrazo_core::image_plane::ImagePlane;
use ingetrazo_gui::IngeTrazoApp;
fn main() {
    let mut rows = serde_json::Map::new();
    for case in [
        "move_relative",
        "rotate_z",
        "rotate_arbitrary",
        "scale_world",
        "scale_axes",
        "scale_zero",
        "transform_partial",
        "transform_redo_capture",
        "detached_move",
    ] {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        let p = ImagePlane::new(
            "scan",
            V::new(0.125, -1.25, 2.5),
            V::new(3.25, 0.125, -0.25),
            V::new(0.375, 2.75, 0.625),
        );
        let id = p.id;
        a.scene.image_planes.push(p);
        a.scene.image_objects.publish(&a.scene.image_planes);
        fn state(a: &IngeTrazoApp, id: uuid::Uuid) -> serde_json::Value {
            let objects = a.scene.image_objects.0.lock().unwrap();
            let p = objects.get(&id).unwrap();
            serde_json::json!({"live":a.scene.image_planes.len(),"frame":[p.origin,p.u,p.v],"undo":a.undo_stack.len(),"redo":a.redo_stack.len()})
        }
        let mut states = vec![state(&a, id)];
        if case == "detached_move" {
            a.delete_image_planes(&[id]);
            states.push(state(&a, id));
        }
        match case {
            "move_relative" | "detached_move" => {
                a.move_image_planes(&[id], V::new(0.7, 1.2, -0.3)).unwrap()
            }
            "rotate_z" => a
                .rotate_image_planes(&[id], V::new(0.25, -2.5, 1.5), -V::Z, 90.)
                .unwrap(),
            "rotate_arbitrary" => a
                .rotate_image_planes(&[id], V::new(0.25, -2.5, 1.5), V::new(1., 2., 3.), 37.5)
                .unwrap(),
            "scale_world" => a
                .scale_image_planes(
                    &[id],
                    V::new(0.25, -2.5, 1.5),
                    V::new(-1.3, 2.25, 0.7),
                    None,
                )
                .unwrap(),
            "scale_axes" => {
                let x = 0.5f64.sqrt();
                a.scale_image_planes(
                    &[id],
                    V::new(0.25, -2.5, 1.5),
                    V::new(1.3, -0.75, 2.5),
                    Some([V::new(x, x, 0.), V::new(-x, x, 0.), V::Z]),
                )
                .unwrap()
            }
            "scale_zero" => a
                .scale_image_planes(&[id], V::new(0.25, -2.5, 1.5), V::new(0., 1.5, -2.), None)
                .unwrap(),
            _ => a
                .transform_image_plane(id, Some(V::new(0.7, 1.2, -0.3)), None, None)
                .unwrap(),
        };
        states.push(state(&a, id));
        if case == "move_relative" {
            a.scene.image_planes[0].origin = (a.scene.image_planes[0].origin.as_vec3()
                + V::new(10.125, -0.5, 3.).as_vec3())
            .as_dvec3();
            a.scene.image_objects.publish(&a.scene.image_planes);
            states.push(state(&a, id));
        }
        if case == "transform_partial" {
            a.scene.image_planes[0].u = V::new(99., 98., 97.);
            a.scene.image_objects.publish(&a.scene.image_planes);
            states.push(state(&a, id));
        }
        a.undo();
        states.push(state(&a, id));
        if case == "transform_redo_capture" {
            a.scene.image_planes[0].origin = V::new(7., 8., 9.);
            a.scene.image_objects.publish(&a.scene.image_planes);
            states.push(state(&a, id));
        }
        a.redo();
        states.push(state(&a, id));
        a.undo();
        states.push(state(&a, id));
        a.redo();
        states.push(state(&a, id));
        if case == "detached_move" {
            a.undo();
            a.undo();
            states.push(state(&a, id));
            a.redo();
            a.redo();
            states.push(state(&a, id));
        }
        rows.insert(case.into(), serde_json::json!(states));
    }
    println!("{}", serde_json::to_string_pretty(&rows).unwrap());
}
