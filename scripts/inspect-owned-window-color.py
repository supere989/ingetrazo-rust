#!/usr/bin/env python3
"""Read only bounded own-window PNG color metadata; never changes pixel verdict."""
import hashlib,json,pathlib,struct,sys,zlib

def inspect(path):
    data=path.read_bytes()
    if len(data)>16*1024*1024 or data[:8]!=b"\x89PNG\r\n\x1a\n":raise ValueError("invalid bounded PNG")
    rows=[];pos=8;profile=None
    while pos<len(data):
        if pos+12>len(data):raise ValueError("truncated PNG")
        n=struct.unpack_from(">I",data,pos)[0];kind=data[pos+4:pos+8];body=data[pos+8:pos+8+n]
        if pos+n+12>len(data)or zlib.crc32(kind+body)!=struct.unpack_from(">I",data,pos+n+8)[0]:raise ValueError("invalid PNG CRC/extent")
        row={"kind":kind.decode("ascii"),"bytes":n}
        if kind==b"iCCP":
            name,packed=body.split(b"\0",1)
            if not packed or packed[0]!=0:raise ValueError("unsupported ICC compression")
            dec=zlib.decompressobj();icc=dec.decompress(packed[1:],65537)
            if len(icc)>65536 or len(icc)<132 or not dec.eof or dec.unconsumed_tail or struct.unpack_from(">I",icc)[0]!=len(icc):raise ValueError("invalid bounded ICC")
            count=struct.unpack_from(">I",icc,128)[0]
            if count>128 or 132+count*12>len(icc):raise ValueError("invalid ICC tag table")
            tags=[]
            for i in range(count):
                tag,off,size=struct.unpack_from(">4sII",icc,132+i*12)
                if size<4 or off+size>len(icc):raise ValueError("invalid ICC tag extent")
                value=icc[off:off+size];item={"tag":tag.decode("ascii"),"type":value[:4].decode("ascii"),"sha256":hashlib.sha256(value).hexdigest()}
                if value[:4]==b"mluc":
                    num,stride=struct.unpack_from(">II",value,8)
                    if num>128 or stride<12 or 16+num*stride>size:raise ValueError("invalid ICC string table")
                    strings=[]
                    for j in range(num):
                        length,offset=struct.unpack_from(">II",value,16+j*stride+4)
                        if offset+length>size:raise ValueError("invalid ICC string")
                        strings.append(value[offset:offset+length].decode("utf-16-be"))
                    item["descriptions"]=sorted(set(strings))
                tags.append(item)
            profile={"name":name.decode("ascii"),"bytes":len(icc),"sha256":hashlib.sha256(icc).hexdigest(),"color_space":icc[16:20].decode("ascii"),"PCS":icc[20:24].decode("ascii"),"tags":tags}
        if kind==b"gAMA":row["gamma"]=struct.unpack(">I",body)[0]/100000
        if kind==b"sRGB":row["intent"]=body[0]
        rows.append(row);pos+=n+12
        if kind==b"IEND":break
    return {"image_sha256":hashlib.sha256(data).hexdigest(),"chunks":rows,"icc":profile,"color_conversion_performed":False,"strict_pixel_verdict_unchanged":True}
if __name__=="__main__":print(json.dumps(inspect(pathlib.Path(sys.argv[1])),indent=2))
