// In-process shaping controls using synthetic sanitized trak tables.
// No platform provider or image acceptance is implied by these tests.
use super::*;
use std::{vec, vec::Vec};

fn table(tracks: &[(f32, &[i16])], sizes: &[f32]) -> Vec<u8> {
    fn u16b(v: &mut Vec<u8>, n: u16) { v.extend(n.to_be_bytes()); }
    fn u32b(v: &mut Vec<u8>, n: u32) { v.extend(n.to_be_bytes()); }
    let mut v=Vec::new(); u32b(&mut v,0x10000);u16b(&mut v,0);
    u16b(&mut v,12);u16b(&mut v,0);u16b(&mut v,0);
    u16b(&mut v,tracks.len() as u16);u16b(&mut v,sizes.len() as u16);
    let size_offset=20+8*tracks.len();u32b(&mut v,size_offset as u32);
    for (i,(track,values)) in tracks.iter().enumerate() {
        assert_eq!(values.len(),sizes.len());
        u32b(&mut v,(*track*65536.) as i32 as u32);u16b(&mut v,0);
        u16b(&mut v,(size_offset+4*sizes.len()+2*sizes.len()*i) as u16);
    }
    for size in sizes {u32b(&mut v,(*size*65536.) as u32);}
    for (_,values) in tracks {for value in *values {u16b(&mut v,*value as u16);}}
    v
}
unsafe extern "C" fn reference(_: *mut hb_face_t, tag: hb_tag_t, user:*mut core::ffi::c_void)->*mut hb_blob_t {
    if tag!=u32::from_be_bytes(*b"trak") {return hb_blob_get_empty();}
    let bytes=&*(user as *const Vec<u8>);
    hb_blob_create(bytes.as_ptr().cast(),bytes.len() as u32,HB_MEMORY_MODE_READONLY,core::ptr::null_mut(),None)
}
unsafe extern "C" fn nominal(_: *mut hb_font_t,_:*mut core::ffi::c_void,cp:u32,glyph:*mut u32,_:*mut core::ffi::c_void)->hb_bool_t {
    *glyph=cp;1
}
unsafe extern "C" fn advance(_: *mut hb_font_t,_:*mut core::ffi::c_void,_:u32,_:*mut core::ffi::c_void)->hb_position_t {6400}
#[derive(Debug,PartialEq)]
struct Pos {gid:u32,cluster:u32,advance:i32,offset:[i32;2]}
fn shape(trak:&mut Vec<u8>, text:&str, ptem:f32, qt:bool)->Vec<Pos>{unsafe{
    let face=hb_face_create_for_tables(Some(reference),(trak as *mut Vec<u8>).cast(),None);
    hb_face_set_upem(face,1000);hb_face_set_glyph_count(face,0x110000);
    let font=hb_font_create(face);hb_font_set_scale(font,6400,6400);hb_font_set_ptem(font,ptem);
    let funcs=hb_font_funcs_create();
    hb_font_funcs_set_nominal_glyph_func(funcs,Some(nominal),core::ptr::null_mut(),None);
    hb_font_funcs_set_glyph_h_advance_func(funcs,Some(advance),core::ptr::null_mut(),None);
    hb_font_set_funcs(font,funcs,core::ptr::null_mut(),None);
    if qt {assert_ne!(ingetrazo_hb_font_set_qt_tracking(font),0);}
    hb_font_make_immutable(font);
    assert_eq!(ingetrazo_hb_font_set_qt_tracking(font),0,"immutable policy mutation must refuse");
    let buffer=hb_buffer_create();hb_buffer_add_utf8(buffer,text.as_ptr().cast(),text.len() as i32,0,text.len() as i32);
    hb_buffer_guess_segment_properties(buffer);hb_shape(font,buffer,core::ptr::null(),0);
    let mut n=0;let info=hb_buffer_get_glyph_infos(buffer,&mut n);let pos=hb_buffer_get_glyph_positions(buffer,&mut n);
    let result=(0..n as usize).map(|i|Pos{gid:(*info.add(i)).codepoint,cluster:(*info.add(i)).cluster,
        advance:(*pos.add(i)).x_advance,offset:[(*pos.add(i)).x_offset,(*pos.add(i)).y_offset]}).collect();
    hb_buffer_destroy(buffer);hb_font_funcs_destroy(funcs);hb_font_destroy(font);hb_face_destroy(face);result
}}
#[test]
fn sole_nonzero_track_clamps_and_interpolates_before_scale(){
    let mut t=table(&[(1.,&[22,4])],&[10.,50.]);
    for (ptem,delta) in [(1.,141),(10.,141),(30.,83),(50.,26),(75.,26)] {
        let old=shape(&mut t,"AB",ptem,false);let qt=shape(&mut t,"AB",ptem,true);
        assert_eq!(old.iter().map(|p|p.advance).collect::<Vec<_>>(),vec![6400,6400]);
        assert_eq!(qt.iter().map(|p|p.advance).collect::<Vec<_>>(),vec![6400+delta,6400+delta]);
        assert_eq!(qt.iter().map(|p|p.offset).collect::<Vec<_>>(),vec![[0,0],[0,0]]);
    }
}
#[test]
fn multiple_tracks_interpolate_at_zero_without_offset(){
    let mut t=table(&[(-1.,&[10]),(1.,&[30])],&[12.]);
    let old=shape(&mut t,"AB",12.,false);let qt=shape(&mut t,"AB",12.,true);
    assert_eq!(old[0].advance,6400);assert_eq!(qt[0].advance,6528);assert_eq!(qt[0].offset,[0,0]);
    let mut t=table(&[(0.,&[21])],&[12.]);
    let old=shape(&mut t,"AB",12.,false);let qt=shape(&mut t,"AB",12.,true);
    assert_eq!(old[0].offset,[64,0]);assert_eq!(qt[0].offset,[0,0]);
    assert_eq!(old[0].advance,qt[0].advance);
}
#[test]
fn tracking_uses_graphemes_and_preserves_shaped_offsets(){
    let mut none=table(&[],&[12.]);let mut t=table(&[(1.,&[4])],&[12.]);
    for text in ["a\u{338}b", "\u{627}\u{64e}\u{644}","\u{915}\u{94d}\u{937}","\u{1f469}\u{200d}\u{1f4bb}"] {
        let baseline=shape(&mut none,text,12.,true);let qt=shape(&mut t,text,12.,true);
        assert_eq!(baseline.len(),qt.len());
        let mut additions=0;
        for (a,b) in baseline.iter().zip(&qt) {
            assert_eq!((a.gid,a.cluster,a.offset),(b.gid,b.cluster,b.offset));
            let d=b.advance-a.advance;assert!(d==0 || d==26,"{text:?}: {baseline:?} versus {qt:?}");
            if [0x338,0x64e,0x94d].contains(&b.gid){assert_eq!(d,0,"combining mark gets no extra tracking: {text:?}: {baseline:?} versus {qt:?}");}
            if d!=0{additions+=1;}
        }
        assert!(additions>0);
    }
}
#[test]
fn empty_and_truncated_tables_preserve_safe_shaping(){
    for mut t in [vec![],vec![0,1],table(&[],&[12.]),table(&[(1.,&[])],&[])] {
        let old=shape(&mut t,"AB",75.,false);let qt=shape(&mut t,"AB",75.,true);assert_eq!(old,qt);
    }
}
