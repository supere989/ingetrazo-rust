#!/usr/bin/env python3
"""Strict reused-original section pixels; never percentage/shape approximation."""
import argparse,json,hashlib,struct,zlib
from pathlib import Path
NAMES=["closed","shared-slab","shared-slab-reversed","transparent","duplicate","coplanar-backface","closed-iso","shared-slab-iso","shared-slab-reversed-iso","closed-front"]
def image(path):
    data=path.read_bytes()
    if len(data)>16*1024*1024 or data[:8]!=b"\x89PNG\r\n\x1a\n":raise ValueError("missing/invalid bounded PNG")
    offset=8;packed=b"";header=None
    while offset<len(data):
        n=struct.unpack_from(">I",data,offset)[0];kind=data[offset+4:offset+8];body=data[offset+8:offset+8+n]
        if offset+12+n>len(data) or zlib.crc32(kind+body)!=struct.unpack_from(">I",data,offset+8+n)[0]:raise ValueError("invalid PNG chunk/CRC")
        if kind==b"IHDR":header=struct.unpack(">IIBBBBB",body)
        if kind==b"IDAT":packed+=body
        offset+=12+n
        if kind==b"IEND":break
    if not header:raise ValueError("missing PNG header")
    w,h,depth,color,compression,filtering,interlace=header
    if not(4<=w<=2048 and 4<=h<=2048 and depth==8 and color in(2,6) and compression==0 and filtering==0 and interlace==0):raise ValueError("unsupported bounded capture PNG encoding")
    channels=3 if color==2 else 4;stride=w*channels;limit=(stride+1)*h
    decoder=zlib.decompressobj();raw=decoder.decompress(packed,limit+1)
    if len(raw)!=limit or decoder.unconsumed_tail or not decoder.eof:raise ValueError("invalid PNG inflated extent")
    prior=bytearray(stride);rows=[];at=0
    for _ in range(h):
        f=raw[at];at+=1;row=bytearray(raw[at:at+stride]);at+=stride
        if f>4:raise ValueError("invalid PNG row filter")
        for i in range(stride):
            a=row[i-channels] if i>=channels else 0;b=prior[i];c=prior[i-channels] if i>=channels else 0
            if f==1:v=a
            elif f==2:v=b
            elif f==3:v=(a+b)//2
            elif f==4:
                p=a+b-c;pa,pb,pc=abs(p-a),abs(p-b),abs(p-c);v=a if pa<=pb and pa<=pc else b if pb<=pc else c
            else:v=0
            row[i]=(row[i]+v)&255
        rows.append(row);prior=row
    assert (w,h)==(800,600), "Unexpected source section extent"
    if channels==4:return b"".join(rows)
    rgba=bytearray(w*h*4);at=0
    for row in rows:
        for x in range(w):
            rgba[at:at+3]=row[x*3:x*3+3];rgba[at+3]=255;at+=4
    return bytes(rgba)
def validate(root,reference):
 report=json.loads((root/"report.json").read_text());assert [r["case"]for r in report["cases"]]==NAMES
 rows=[]
 for row in report["cases"]:
  name=row["case"];assert row["warm_frames"]==3
  for counter in ["target_allocations","geometry_uploads","section_fill_uploads","section_fill_upload_bytes","section_contour_pipeline_builds"]:assert row[counter+"_before"]==row[counter+"_after"]
  assert row["contours_enabled"] is True and row["fill_off_contours_enabled"] is True and row["cuts_off_contours_enabled"] is False
  assert row["section_contour_pipeline_builds_before"]==1 and row["fill_off_contour_pipeline_builds"]==2 and row["cuts_off_contour_pipeline_builds"]==2
  source=image(reference/(name+".png"));native=(root/(name+".rgba")).read_bytes();disabled=(root/(name+"-disabled.rgba")).read_bytes();assert len(native)==len(source)==len(disabled)==800*600*4
  diff=sum(source[i:i+4]!=native[i:i+4]for i in range(0,len(source),4))
  mask=lambda b:[b[i]>180 and b[i+2]>180 and b[i+1]<80 for i in range(0,len(b),4)]
  a,b,c=mask(source),mask(native),mask(disabled);mdiff=sum(x!=y for x,y in zip(a,b));assert not any(c),(name,"disabled fill still visible")
  rows.append({"case":name,"rgba_mismatch_pixels":diff,"source_cap_mask_pixels":sum(a),"native_cap_mask_pixels":sum(b),"cap_mask_mismatch_pixels":mdiff,"native_sha256":hashlib.sha256(native).hexdigest()})
 return {"cases":rows,"passed":all(r["rgba_mismatch_pixels"]==0 for r in rows),"scope":"All source/native RGBA pixels, including retained source contour. Direct GPU contour candidate included; any mismatch stays failed. Cap mask diagnostics are additional, not a relaxed acceptance."}
def main():
 p=argparse.ArgumentParser();p.add_argument("native",type=Path);p.add_argument("--reference",type=Path,default=Path("docs/audits/section-original-gl"));p.add_argument("--output",type=Path);args=p.parse_args();result=validate(args.native,args.reference);text=json.dumps(result,indent=2)+"\n";print(text)
 if args.output:args.output.write_text(text)
 raise SystemExit(0 if result["passed"]else 1)
if __name__=="__main__":main()
