use ingetrazo_formats::sheet_dxf::{write_view_dxf, ViewLayer};
fn main() {
 let output=std::env::args().nth(1).expect("output DXF path");
 let edge=[[0.,0.,1.23456789123,-2.],[1e-10,-1e10,0.,0.]];
 let hidden=[[2.,3.,4.,5.]];
 let groups=[ViewLayer{name:"  Planta General ",segments:&edge,dashed:false},ViewLayer{name:"Planta General-PERFIL",segments:&[],dashed:false},ViewLayer{name:"Planta General-CORTE",segments:&[],dashed:false},ViewLayer{name:"Planta General-OCULTAS",segments:&hidden,dashed:true}];
 let count=write_view_dxf(std::fs::File::create(&output).unwrap(),&groups).unwrap();
 assert_eq!(count,3);
 let mut out=vec![];let bad=[[f64::NAN,0.,0.,0.]];
 assert!(write_view_dxf(&mut out,&[ViewLayer{name:"bad",segments:&bad,dashed:false}]).is_err());assert!(out.is_empty());
 let path=std::path::Path::new(&output);let before=std::fs::read(path).unwrap();
 assert!(ingetrazo_formats::sheet_dxf::save_view_dxf(path,&[ViewLayer{name:"bad",segments:&bad,dashed:false}]).is_err());assert_eq!(before,std::fs::read(path).unwrap());

}
