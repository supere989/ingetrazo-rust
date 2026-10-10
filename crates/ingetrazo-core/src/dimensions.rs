// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! 3D Linear dimensions and leader text annotations.

use glam::DVec3;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::units::{format_length, LengthUnit};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dimension {
    #[serde(skip)] pub measurement_frame:glam::DMat4,
    #[serde(skip)] pub anchor_a:Option<crate::dimension_anchors::VertexAnchor>,
    #[serde(skip)] pub anchor_b:Option<crate::dimension_anchors::VertexAnchor>,
    #[serde(default)] pub native_document:serde_json::Value,
    #[serde(default)] pub native_baseline:serde_json::Value,
    #[serde(default)] pub layer:Option<String>,
    #[serde(default)] pub axis:Option<String>,
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    #[serde(alias = "a")]
    pub start: DVec3,
    #[serde(alias = "b")]
    pub end: DVec3,
    #[serde(default)]
    pub offset: DVec3,
    #[serde(default)]
    #[serde(alias="text")]
    pub text_override: Option<String>,
}

impl Dimension {
    pub fn new(start: DVec3, end: DVec3, offset: DVec3) -> Self {
        Self {
            measurement_frame:glam::DMat4::IDENTITY,
            anchor_a:None,anchor_b:None,
            native_document:serde_json::Value::Null,native_baseline:serde_json::Value::Null,layer:None,axis:None,
            id: Uuid::new_v4(),
            start,
            end,
            offset,
            text_override: None,
        }
    }

    fn linear_frame(start:DVec3,end:DVec3,axis:Option<&str>)->Option<(DVec3,DVec3)> {
        let u=match axis{Some("x")=>DVec3::X,Some("y")=>DVec3::Y,Some("z")=>DVec3::Z,_=>return None};
        let ab=end-start;if ab.length()<1e-9{return None;}let d=ab.normalize();let e=d-u*d.dot(u);if e.length()<1e-6{return None;}Some((u,e.normalize()))
    }
    pub fn line_points(&self)->(DVec3,DVec3) {
        let start=self.measurement_frame.transform_point3(self.start);let end=self.measurement_frame.transform_point3(self.end);let offset=self.measurement_frame.transform_vector3(self.offset);
        let(first,last)=if let Some((_,e))=Self::linear_frame(start,end,self.axis.as_deref()){let a=offset.dot(e);let b=a-(end-start).dot(e);(start+e*a,end+e*b)}else{let d=(end-start).normalize_or_zero();let offset=offset-d*offset.dot(d);(start+offset,end+offset)};
        let inverse=self.measurement_frame.inverse();(inverse.transform_point3(first),inverse.transform_point3(last))
    }
    pub fn placement_in_frame(start:DVec3,end:DVec3,cursor:DVec3,frame:glam::DMat4)->(DVec3,Option<String>){let(offset,axis)=Self::placement_for_cursor(frame.transform_point3(start),frame.transform_point3(end),frame.transform_point3(cursor));(frame.inverse().transform_vector3(offset),axis)}
    pub fn placement_for_cursor(start:DVec3,end:DVec3,cursor:DVec3)->(DVec3,Option<String>) {
        let ab=end-start;let length=ab.length();if length<1e-9{return(cursor-start,None);}
        let d=ab/length;let aligned=(cursor-start)-d*(cursor-start).dot(d);let w=cursor-(start+end)*0.5;
        if w.dot(d).abs() <= 20_f64.to_radians().tan()*(w-d*w.dot(d)).length(){return(aligned,None);}
        let mut best=None;let mut beyond=0;let lo=15_f64.to_radians().sin();let hi=15_f64.to_radians().cos();
        for name in ["x","y","z"]{if let Some((u,e))=Self::linear_frame(start,end,Some(name)){
            let along=d.dot(u).abs();if along<lo||along>hi{continue;}let t=(cursor-start).dot(e);let span=ab.dot(e);let low=0_f64.min(span);let high=0_f64.max(span);
            let over=if t<low{low-t}else if t>high{t-high}else{0.};if over<=1e-9{continue;}beyond+=1;
            if best.as_ref().map(|(old,_,_)|over>*old).unwrap_or(true){best=Some((over,name.to_owned(),e*t));}
        }}
        if beyond==1{if let Some((_,name,offset))=best{return(offset,Some(name));}}(aligned,None)
    }

    pub fn length(&self) -> f64 {
        let delta=self.measurement_frame.transform_vector3(self.end-self.start);
        match self.axis.as_deref(){Some("x")=>delta.x.abs(),Some("y")=>delta.y.abs(),Some("z")=>delta.z.abs(),_=>delta.length()}
    }

    pub fn display_text(&self, unit: LengthUnit, decimals: usize) -> String {
        if let Some(ref text) = self.text_override {
            text.replace("<>",&format_length(self.length(),unit,decimals))
        } else {
            format_length(self.length(), unit, decimals)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_placement_matches_linear_side_above_and_aligned_corridor() {
        let a=DVec3::ZERO;let b=DVec3::new(3.,4.,0.);
        let (offset,axis)=Dimension::placement_for_cursor(a,b,DVec3::new(6.,1.,0.));assert_eq!(axis.as_deref(),Some("y"));
        let mut dimension=Dimension::new(a,b,offset);dimension.axis=axis;assert_eq!(dimension.length(),4.);
        let (first,last)=dimension.line_points();assert!((first-DVec3::new(6.,0.,0.)).length()<1e-10);assert!((last-DVec3::new(6.,4.,0.)).length()<1e-10);
        let (_,axis)=Dimension::placement_for_cursor(a,b,DVec3::new(1.,6.,0.));assert_eq!(axis.as_deref(),Some("x"));
        let (offset,axis)=Dimension::placement_for_cursor(a,b,DVec3::new(-4.,5.,0.));assert!(axis.is_none());assert!(offset.dot(b).abs()<1e-10);
    }
    #[test]
    fn test_dimension_deserialize_ab_format() {
        let json_str = r#"{"a": [0.0, 0.0, 0.0], "b": [0.0, 0.0, 2.4], "offset": [-1.0, 0.0, 0.0]}"#;
        let dim: Dimension = serde_json::from_str(json_str).expect("deserialize dimension");
        assert_eq!(dim.start, DVec3::new(0.0, 0.0, 0.0));
        assert_eq!(dim.end, DVec3::new(0.0, 0.0, 2.4));
        assert_eq!(dim.offset, DVec3::new(-1.0, 0.0, 0.0));
        assert!((dim.length() - 2.4).abs() < 1e-6);
    }
}
