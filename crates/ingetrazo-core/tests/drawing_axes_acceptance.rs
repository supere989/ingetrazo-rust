use glam::{DMat4,DVec3};
use ingetrazo_core::{drawing_axes::DrawingAxes,scene::Scene};

fn vector(value:&serde_json::Value)->DVec3 {
    let v=value.as_array().unwrap();DVec3::new(v[0].as_f64().unwrap(),v[1].as_f64().unwrap(),v[2].as_f64().unwrap())
}

#[test]
fn actual_original_reentry_frames_match_axes_without_reencoding() {
    let evidence:serde_json::Value=serde_json::from_str(include_str!("../../../docs/audits/component-reentry-source.json")).unwrap();
    for case in evidence["cases"].as_array().unwrap() {
        let data=case["reentry_drawing_frame"].as_array().unwrap().iter().map(|v|v.as_f64().unwrap()).collect::<Vec<_>>();
        let raw:[f64;16]=data.try_into().unwrap();let frame=DMat4::from_cols_array(&raw);
        let axes=DrawingAxes::from_frame(Some(frame));
        for (native,original) in [(axes.origin,vector(&case["origin"])),(axes.x,vector(&case["axes"]["x"])),(axes.y,vector(&case["axes"]["y"])),(axes.z,vector(&case["axes"]["z"]))] {
            assert!(native.distance(original)<3e-7,"{}: {native:?} vs {original:?}",case["kind"]);
        }
        let (u,v)=axes.plane_axes(DVec3::Z);
        assert!(u.distance(vector(&case["plane_z_u"]))<3e-7);
        assert!(v.distance(vector(&case["plane_z_v"]))<3e-7);
        assert_eq!(frame.to_cols_array(),raw,"the source affine data is never orthonormalized in storage");
    }
}

#[test]
fn world_reset_and_reflected_skew_are_not_stale_or_normalized_storage() {
    assert_eq!(DrawingAxes::from_frame(None),DrawingAxes::default());
    let raw=DMat4::from_cols(glam::DVec4::new(2.,0.,0.,0.),glam::DVec4::new(1.,3.,0.,0.),glam::DVec4::new(0.,0.,-4.,0.),glam::DVec4::new(8.,9.,10.,1.));
    let axes=DrawingAxes::from_frame(Some(raw));assert_eq!(axes.x,DVec3::X);assert_eq!(axes.y,DVec3::Y);assert_eq!(axes.z,-DVec3::Z);
    let (u,v)=axes.plane_axes(DVec3::X);assert_eq!(u,DVec3::Y);assert_eq!(v,DVec3::Z);
    let mut scene=Scene::new();let baseline=serde_json::to_value(&scene).unwrap();scene.drawing_frame=Some(raw);
    assert_eq!(scene.drawing_axes(),axes);assert_eq!(serde_json::to_value(&scene).unwrap(),baseline);
    scene.drawing_frame=None;assert_eq!(scene.drawing_axes(),DrawingAxes::default());
}
