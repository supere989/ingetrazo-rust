//! Produce the actual shared production paint operation for independent Qt comparison.
use ingetrazo_render::cad2d::EncodedRgba;
fn main()->Result<(),Box<dyn std::error::Error>>{let path=std::env::args_os().nth(1).ok_or("output binary path required")?;let mut bytes=Vec::with_capacity(65536);for alpha in 0..=255u8{for channel in 0..=255u8{let p=EncodedRgba::from_bytes([channel,channel,channel,alpha],1.)?.qt_premultiplied8()?;bytes.push((p[0]*255.).round()as u8);}}std::fs::write(path,bytes)?;Ok(())}
