"""Reject incomplete screenshot files while capture writers are active."""
from pathlib import Path
import struct,zlib

def valid_png(path):
 try:
  data=Path(path).read_bytes()
  if data[:8]!=b"\x89PNG\r\n\x1a\n":return False
  at=8;packed=b"";dimensions=None;ended=False
  while at<len(data):
   if at+12>len(data):return False
   n=struct.unpack_from(">I",data,at)[0]
   if at+12+n>len(data):return False
   tag=data[at+4:at+8];body=data[at+8:at+8+n]
   if zlib.crc32(tag+body)&0xffffffff!=struct.unpack_from(">I",data,at+8+n)[0]:return False
   at+=12+n
   if tag==b"IHDR":
    if n!=13:return False
    dimensions=struct.unpack_from(">IIBBBBB",body)
   if tag==b"IDAT":packed+=body
   if tag==b"IEND":ended=True;break
  if not ended or at!=len(data) or not dimensions or not packed:return False
  w,h,depth,color,_,_,interlace=dimensions
  if w<=0 or h<=0 or depth!=8 or color!=6 or interlace!=0:return False
  return len(zlib.decompress(packed))==h*(1+w*4)
 except (OSError,ValueError,struct.error,zlib.error):return False

