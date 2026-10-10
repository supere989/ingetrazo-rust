#!/usr/bin/env python3
"""Independent distributed projected-wire pixel protocol, never state-only acceptance."""
import argparse,json,math,statistics
from pathlib import Path
from PySide6.QtGui import QImage
REQUIRED=['group-before','group-live-preview','group-copy-preview','group-copy-commit','group-array-undo']
def pixel(im,x,y):
 c=im.pixelColor(round(x),round(y));return [c.red(),c.green(),c.blue()]
def contrast(a,b):return sum((x-y)**2 for x,y in zip(a,b))**.5

def wire_coverage(im,segments):
 assert len(segments)>=12,'Incomplete committed/preview path'
 hits=[]
 for a,b in segments:
  dx,dy=b[0]-a[0],b[1]-a[1];length=math.hypot(dx,dy);assert length>1
  normal=[-dy/length,dx/length];samples=[]
  for t in [.25,.5,.75]:
   x,y=a[0]+dx*t,a[1]+dy*t
   assert 10<=x<im.width()-10 and 10<=y<im.height()-10,'Off-image wire cannot prove visibility'
   backgrounds=[pixel(im,x+normal[0]*d,y+normal[1]*d)for d in [-8,-6,6,8]]
   bg=[statistics.median(c[k]for c in backgrounds)for k in range(3)]
   strongest=max(contrast(pixel(im,x+ox,y+oy),bg)for ox in [-2,-1,0,1,2]for oy in [-2,-1,0,1,2])
   samples.append(strongest>=25)
  hits.append(sum(samples)>=2)
 # Distributed coverage, so isolated cursor/endpoint/UI marks cannot pass.
 quarters=[hits[i*len(hits)//4:(i+1)*len(hits)//4]for i in range(4)]
 return {'hit_segments':sum(hits),'segments':len(hits),'fraction':sum(hits)/len(hits),'quarters':[sum(q)/len(q)for q in quarters]}

def inspect(root,manifest):
 assert manifest['protocol']=='projected-wire-pixels-v1'
 assert manifest['mode']in ['ordinary','linked','nested']
 assert [p['phase']for p in manifest['phases']]==REQUIRED,'Missing committed/cancel/preview capture'
 results=[]
 for phase in manifest['phases']:
  path=root/phase['image'];assert path.is_file()and path.stat().st_size>0
  image=QImage(str(path));assert not image.isNull(),'Unreadable or empty capture'
  assert [image.width(),image.height()]==manifest['image_size']
  paths=phase['wires'];assert paths,'No authored projected path'
  assert len({w['id']for w in paths})==len(paths),'Duplicate wire identity'
  bounds=phase['viewport_rect'];assert len(bounds)==4 and bounds[2]>0 and bounds[3]>0
  for wire in paths:
   for segment in wire['segments']:
    for x,y in segment:assert bounds[0]+10<=x<bounds[0]+bounds[2]-10 and bounds[1]+10<=y<bounds[1]+bounds[3]-10,'Wire outside actual world viewport'
  if phase['phase']in ['group-copy-preview','group-copy-commit']:
   centers=[[statistics.mean(p[k]for seg in w['segments']for p in seg)for k in range(2)]for w in paths]
   assert max(math.dist(a,b)for a in centers for b in centers)>=8,'Duplicate spatial wire cannot prove copy visibility'
  roles=[w['role']for w in paths]
  assert 'committed'in roles,'Missing actual committed wire observation'
  if phase['phase']=='group-copy-preview':assert 'preview'in roles,'Missing actual preview wire observation'
  if phase['phase']=='group-copy-commit':assert roles.count('committed')>=2,'Missing newly committed copy wire'
  for wire in paths:assert wire['role']in ['committed','preview']
  for wire in paths:
   value=wire_coverage(image,wire['segments']);passed=value['fraction']>=.75 and min(value['quarters'])>=.5
   results.append({'phase':phase['phase'],'wire':wire['id'],'passed':passed,**value})
 return {'passed':all(r['passed']for r in results),'cases':results,'scope':'Distributed visible ink on independently projected committed/preview wire paths; no state-transition-only acceptance'}

def main():
 p=argparse.ArgumentParser();p.add_argument('manifest');p.add_argument('output');a=p.parse_args();path=Path(a.manifest)
 result=inspect(path.parent,json.loads(path.read_text()));Path(a.output).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));raise SystemExit(0 if result['passed']else 1)
if __name__=='__main__':main()
