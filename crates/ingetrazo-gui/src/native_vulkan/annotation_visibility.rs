pub(crate) use ingetrazo_render::annotation_query::{source_threshold,Query,prepare_query,map};
#[cfg(test)]fn ceil_f32(value:f64)->f32{let v=value as f32;if (v as f64)<value{v.next_up()}else{v}}
/// Diagnostic-only CPU reconstruction for comparison with frozen Qt operation
/// cases. Production visibility reads depth on GPU; this never reads a frame.
fn sampled_occluded(query:Query, depth:f32, inverse:&[f32;16], eye:[f32;3], forward:[f32;3])->bool {
    if query.active==0 || depth>=1. {return false;}
    let q=map(inverse,[query.normalized[0],query.normalized[1],(depth as f64*2.-1.) as f32,1.]);
    if (q[3] as f64).abs()<1e-12{return false;}
    let surface=std::array::from_fn::<_,3,_>(|i|(q[i] as f64/q[3] as f64) as f32-eye[i]);
    let along=surface[0]*forward[0]+surface[1]*forward[1]+surface[2]*forward[2];
    along<query.threshold
}
use bytemuck::Zeroable;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gpu_float_comparison_preserves_source_double_boundary() {
        for point in [-1e30_f32,-1000.,-1.,-0.021,0.,0.019,0.021,1.,100.,1000.,1e30] {
            let source = point as f64 - 0.02_f64.max(0.003 * (point as f64).abs());
            let boundary = source_threshold(point).unwrap();
            for surface in [boundary.next_down(),boundary,boundary.next_up(),point] {
                assert_eq!(surface < boundary, (surface as f64) < source,
                    "point={point:?}, surface={surface:?}, source={source:?}");
            }
        }
        assert!(source_threshold(f32::NAN).is_err());
        assert!(source_threshold(f32::INFINITY).is_err());
    }
    #[test]
    fn nearest_rounding_would_hide_a_source_visible_boundary() {
        let threshold = 1.0_f64 + f32::EPSILON as f64 * 0.25;
        let nearest = threshold as f32;
        assert!((nearest as f64) < threshold);
        assert!(!(nearest < nearest));
        assert!(nearest < ceil_f32(threshold));
    }
    #[test]
    fn inverse_w_shader_literal_is_the_source_double_ceiling() {
        let nearest = 1e-12_f64 as f32;
        let boundary = ceil_f32(1e-12_f64);
        assert_eq!(nearest.to_bits(), 0x2b8cbccc);
        assert_eq!(boundary.to_bits(), 0x2b8cbccd);
        for magnitude in [nearest.next_down(), nearest, boundary, boundary.next_up()] {
            for w in [magnitude, -magnitude] {
                assert_eq!(w.abs() < boundary, (w as f64).abs() < 1e-12_f64);
            }
        }
        for shader in [
            include_str!("shaders/annotation_visibility.comp"),
            include_str!("shaders/annotation_visibility_single.comp"),
        ] {
            assert!(shader.contains("uintBitsToFloat(0x2b8cbccdu)"));
        }
    }
    #[test]
    fn source_negative_fraction_is_admitted_but_whole_negative_pixel_is_not() {
        let m=glam::Mat4::IDENTITY.to_cols_array();
        let a=prepare_query(&m,[-1.1,0.,0.2],[0.;3],[0.,0.,1.],[5,5]).unwrap();
        assert_eq!(a.active,1);assert_eq!(a.pixel,[0,2]);
        let b=prepare_query(&m,[-1.5,0.,0.2],[0.;3],[0.,0.,1.],[5,5]).unwrap();
        assert_eq!(b.active,0);
        assert!(prepare_query(&m,[f32::NAN,0.,0.],[0.;3],[0.,0.,1.],[5,5]).is_err());
    }

    #[test]
    fn retained_actual_qt_depth_cases_match_prepared_queries_and_reconstruction() {
        let oracle:serde_json::Value=serde_json::from_str(include_str!("../../../../docs/audits/native-annotation-source/depth-original.json")).unwrap();
        let array=|v:&serde_json::Value|v.as_array().unwrap().iter().map(|x|x.as_f64().unwrap() as f32).collect::<Vec<_>>();
        let rows=oracle["cases"].as_array().unwrap();assert_eq!(rows.len(),18);
        for row in rows {
            if !row["fallback"].is_null(){assert_eq!(row["fallback_calls"].as_array().unwrap().len(),1);continue;}
            let m: [f32;16]=array(&row["mvp_column_major"]).try_into().unwrap();
            let inverse:[f32;16]=array(&row["inverse_mvp_column_major"]).try_into().unwrap();
            let world:[f32;3]=array(&row["world"]).try_into().unwrap();
            let eye:[f32;3]=array(&row["eye"]).try_into().unwrap();
            let forward:[f32;3]=array(&row["forward"]).try_into().unwrap();
            let query=prepare_query(&m,world,eye,forward,[5,5]).unwrap();
            let mut depth=0_f32;
            if query.active!=0 {
                for y in (query.pixel[1]-1).max(0)..=(query.pixel[1]+1).min(4) {
                    for x in (query.pixel[0]-1).max(0)..=(query.pixel[0]+1).min(4) {
                        depth=depth.max(row["depth"][y as usize][x as usize].as_f64().unwrap() as f32);
                    }
                }
            }
            assert_eq!(sampled_occluded(query,depth,&inverse,eye,forward),row["occluded"].as_bool().unwrap(),"{}",row["name"]);
        }
    }

}
