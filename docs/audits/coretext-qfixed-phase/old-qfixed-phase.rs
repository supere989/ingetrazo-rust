fn device_position_with_count(x:i64,y:i64,count:u8)->(f64,f64,[u8;2]){
 // QFontEngine divides by QFixed(count), whose division rounds nearest.
 // Integer division here incorrectly maps phase2/3 to42 instead of43.
 let phase=if count<=1{0}else{let count=i64::from(count);let bucket=(x.rem_euclid(64)+1)*count/64;bucket*64/count};
 (x.div_euclid(64)as f64+(phase/64)as f64,(y+32).div_euclid(64)as f64,[(phase%64)as u8,0])
}
fn main(){for c in 1..=12 {for pixel in [-4,-1,0,1,4] {for frac in 0..64 {for y in [-33,-32,-1,0,31,32,63] {let x=pixel*64+frac;let(fx,fy,p)=device_position_with_count(x,y,c);println!("{x},{y},{c},{},{},{}",fx as i64,fy as i64,p[0]);}}}}}
