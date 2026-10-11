use ingetrazo_formats::sheet_image::{Metrics,Format,resolve_path,save_rgba};
use std::path::PathBuf;
fn main()->Result<(),Box<dyn std::error::Error>>{
    let args=std::env::args().skip(1).collect::<Vec<_>>();
    let source=PathBuf::from(&args[0]);let out=PathBuf::from(&args[1]);std::fs::create_dir_all(&out)?;
    let original:serde_json::Value=serde_json::from_slice(&std::fs::read(source.join("source.json"))?)?;
    let mut rows=Vec::new();
    for row in original["images"].as_array().unwrap(){
        if args.get(2).is_some_and(|selected| row["case"].as_str()!=Some(selected.as_str())) {continue;}
        let paper=match row["paper"].as_str().unwrap(){"A4"=>[210.,297.],"A3"=>[297.,420.],_=>unreachable!()};
        let page=if row["landscape"].as_bool().unwrap(){[paper[1],paper[0]]}else{paper};
        let metrics=Metrics::new(page,row["dpi"].as_u64().unwrap()as u16)?;
        let file=row["image"]["file"].as_str().unwrap();
        // Encode the same first-generation painter pixels, never recompress the source JPEG.
        let input=if row["case"]=="shapes-jpeg" {"shapes.png"} else {file};
        let pixels=image::open(source.join(input))?.to_rgba8();
        save_rgba(&out.join(file),metrics,pixels.as_raw())?;
        rows.push(serde_json::json!({"case":row["case"],"metrics":metrics,"file":file,"input":input}));
    }
    let paths=[("chosen",Format::Png),("chosen-jpeg",Format::Jpeg),("upper.JPG",Format::Png),(".png",Format::Jpeg)].map(|(path,format)|{let(p,f)=resolve_path(std::path::Path::new(path),format);serde_json::json!({"input":path,"path":p,"format":f})});
    let sentinel=out.join("preserve.png");std::fs::write(&sentinel,b"destination sentinel")?;
    let metrics=Metrics::new([1.,1.],50)?;let mut pixels=vec![255;metrics.rgba_bytes()?];pixels[3]=0;
    let rejected=save_rgba(&sentinel,metrics,&pixels).is_err();assert!(rejected);assert_eq!(std::fs::read(&sentinel)?,b"destination sentinel");
    let no_temps=std::fs::read_dir(&out)?.all(|e|!e.unwrap().file_name().to_string_lossy().starts_with(".ingetrazo-image-"));assert!(no_temps);
    std::fs::write(out.join("native.json"),serde_json::to_vec_pretty(&serde_json::json!({"rows":rows,"paths":paths,"nonopaque_refused":rejected,"destination_preserved":true,"no_temps":no_temps,"scope":"Codec contract uses frozen source pixels; native rendering and GUI integration remain pending"}))?)?;Ok(())
}
