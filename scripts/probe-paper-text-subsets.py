#!/usr/bin/env python3
"""Read actual original QPdfWriter embedded subset tables; no replacement painter."""
import sys,re,zlib,struct,json,hashlib
from pathlib import Path
root=Path(sys.argv[1]);out=Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True)
names=['accents','bold','combining','italic','kerns','latin','mono','serif','serif_bold']
def read_font(data):
 n=struct.unpack_from('>H',data,4)[0];tables={}
 for i in range(n):
  tag,checksum,offset,length=struct.unpack_from('>4sIII',data,12+16*i);assert offset+length<=len(data);tables[tag.decode()]=data[offset:offset+length]
 head=tables['head'];em=struct.unpack_from('>H',head,18)[0];long=struct.unpack_from('>h',head,50)[0];assert long in (0,1)
 count=struct.unpack_from('>H',tables['maxp'],4)[0];fmt='I'if long else'H';loc=struct.unpack('>'+str(count+1)+fmt,tables['loca']);factor=1 if long else 2;glyphs=[]
 for gid in range(count):
  g=tables['glyf'][loc[gid]*factor:loc[gid+1]*factor]
  if not g:glyphs.append({'cid':gid,'empty':True});continue
  contours,*bbox=struct.unpack_from('>hhhhh',g);assert contours>0
  ends=struct.unpack_from('>'+str(contours)+'H',g,10);points=ends[-1]+1;offset=10+2*contours;instructions=struct.unpack_from('>H',g,offset)[0];offset+=2+instructions;flags=[]
  while len(flags)<points:
   f=g[offset];offset+=1;flags.append(f)
   if f&8:
    repeat=g[offset];offset+=1;flags.extend([f]*repeat)
  assert len(flags)==points;xy=[[],[]]
  for axis in range(2):
   value=0
   for f in flags:
    short=f&(2 if axis==0 else 4);same=f&(16 if axis==0 else 32);delta=0
    if short:delta=g[offset]*(1 if same else -1);offset+=1
    elif not same:delta=struct.unpack_from('>h',g,offset)[0];offset+=2
    value+=delta;xy[axis].append(value)
  glyphs.append({'cid':gid,'bounds':bbox,'ends':list(ends),'instructions':instructions,'points':[[x,y,f&1]for x,y,f in zip(*xy,flags)]})
 return {'em':em,'tables':{k:{'bytes':len(v),'sha256':hashlib.sha256(v).hexdigest()}for k,v in tables.items()},'glyphs':glyphs}
rows=[]
for name in names:
 data=(root/(name+'.original.pdf')).read_bytes();objects={};streams={}
 for m in re.finditer(rb'(\d+) 0 obj\s*(.*?)endobj',data,re.S):
  index,body=m.groups();index=int(index);objects[index]=body
 for index,body in objects.items():
  if b'stream' in body:
   header,tail=body.split(b'stream',1);tail=tail[2:]if tail.startswith(b'\r\n')else tail[1:]
   indirect=re.search(rb'/Length\s+(\d+) 0 R',header)
   length=int(objects[int(indirect[1])].strip())if indirect else int(re.search(rb'/Length\s+(\d+)',header)[1])
   payload=tail[:length];assert len(payload)==length
   if b'/FlateDecode' in header:payload=zlib.decompress(payload)
   streams[index]=payload
 descriptors=[(i,b)for i,b in objects.items()if b'/FontDescriptor' in b and b'/FontFile2' in b];assert len(descriptors)==1
 _,descriptor=descriptors[0];font_index=int(re.search(rb'/FontFile2\s+(\d+) 0 R',descriptor)[1]);font=read_font(streams[font_index]);content=[s.decode('latin1')for s in streams.values()if b'BT' in s];assert len(content)==1
 cids=[int(v,16)for v in re.findall(r'<([0-9A-F]+)> Tj',content[0])];assert all(v<len(font['glyphs'])for v in cids)
 rows.append({'case':name,'pdf_sha256':hashlib.sha256(data).hexdigest(),'font_program_sha256':hashlib.sha256(streams[font_index]).hexdigest(),'cids_in_draw_order':cids,'font':font})
(out/'actual-original-subset-controls.json').write_text(json.dumps({'scope':'Actual embedded original font subset glyphs, not font/layout implementation','cases':rows},indent=2)+'\n')
print('PASS nine actual original PDF subset intermediate records extracted; no native subset parity claim')
