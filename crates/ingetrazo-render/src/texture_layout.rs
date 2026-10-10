//! Native texture byte layouts, shared by Vulkan and Metal resource owners.
//! BC7/BC5 describe real encoded blocks; selecting them is not a compressor or
//! a lossless RGBA promise. Source-exact authored color defaults to RGBA8.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum Format {Rgba8,Bc5,Bc7}
impl Format {
 pub fn block(self)->(u32,u32,u32){match self{Self::Rgba8=>(1,1,4),Self::Bc5|Self::Bc7=>(4,4,16)}}
 pub fn mip_bytes(self,width:u32,height:u32)->Result<u64,String>{
  if width==0||height==0{return Err("Texture dimensions must be nonzero".into())}
  let (bw,bh,bytes)=self.block();let x=(width as u64).div_ceil(bw as u64);let y=(height as u64).div_ceil(bh as u64);x.checked_mul(y).and_then(|v|v.checked_mul(bytes as u64)).ok_or_else(||"Texture byte extent overflow".into())
 }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub struct Mip {pub width:u32,pub height:u32,pub offset:u64,pub bytes:u64}
#[derive(Clone,Debug,PartialEq,Eq)]pub struct Layout {pub format:Format,pub mips:Vec<Mip>,pub bytes:u64}
impl Layout {
 pub fn new(format:Format,width:u32,height:u32,levels:u32)->Result<Self,String>{
  if width==0||height==0||levels==0||levels>32-width.max(height).leading_zeros(){return Err("Texture mip count outside dimension range".into())}
  let mut bytes=0u64;let mut mips=Vec::with_capacity(levels as usize);
  for level in 0..levels{let w=(width>>level).max(1);let h=(height>>level).max(1);let size=format.mip_bytes(w,h)?;mips.push(Mip{width:w,height:h,offset:bytes,bytes:size});bytes=bytes.checked_add(size).ok_or("Texture mip chain extent overflow")?;}
  Ok(Self{format,mips,bytes})
 }
 pub fn full(format:Format,width:u32,height:u32)->Result<Self,String>{Self::new(format,width,height,32-width.max(height).leading_zeros())}
}
#[cfg(test)]mod tests {
 use super::*;
 #[test]fn source_rgba_non_power_of_two_chain_has_exact_offsets(){let l=Layout::full(Format::Rgba8,7,5).unwrap();assert_eq!(l.mips,vec![Mip{width:7,height:5,offset:0,bytes:140},Mip{width:3,height:2,offset:140,bytes:24},Mip{width:1,height:1,offset:164,bytes:4}]);assert_eq!(l.bytes,168)}
 #[test]fn bc_blocks_round_up_including_small_mip_tail(){for f in [Format::Bc5,Format::Bc7]{let l=Layout::full(f,7,5).unwrap();assert_eq!(l.mips.iter().map(|m|m.bytes).collect::<Vec<_>>(),[64,16,16]);assert_eq!(l.bytes,96)}}
 #[test]fn rejects_empty_extra_levels_and_overflow_without_allocating(){assert!(Layout::full(Format::Rgba8,0,0).is_err());assert!(Layout::new(Format::Rgba8,1,1,2).is_err());assert!(Format::Rgba8.mip_bytes(u32::MAX,u32::MAX).is_err());assert!(Layout::full(Format::Bc7,u32::MAX,u32::MAX).is_err());}
}
