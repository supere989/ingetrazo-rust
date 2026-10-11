#[cfg(target_os="macos")]
fn main()->Result<(),String>{let mut args=std::env::args_os().skip(1);let input=args.next().ok_or("input IGZ required")?.into();let output=args.next().ok_or("output directory required")?.into();ingetrazo_gui::native_metal::run_billboard_probe(input,output)}
#[cfg(not(target_os="macos"))]
fn main(){eprintln!("Native Metal requires macOS");}
