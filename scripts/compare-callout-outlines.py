#!/usr/bin/env python3
"""Read-only exact boundary comparison; never alters or generates raster geometry."""
import collections,json,math,sys
from pathlib import Path
source=json.loads(Path(sys.argv[1]).read_text());witness=json.loads(Path(sys.argv[2]).read_text());native=json.loads(Path(sys.argv[3]).read_text());out=Path(sys.argv[4])
assert witness['full_mask_equivalent']is True and witness['different_rgba_pixels']==0
assert source['source_pin']==witness['source_pin']==native['source_pin'] and source['case']==witness['case']==native['case']=='llamadas-default'
st=source['stages'][0];heavy=native['heavy'];matrix=st['combined_transform'];assert matrix[1]==matrix[2]==matrix[3]==matrix[5]==0 and matrix[8]==1 and matrix[0]==matrix[4]==heavy['density']
assert matrix[6:8]==heavy['translation_px'] and heavy['clip']is None and heavy['clip_intersection']is None
assert witness['actual_rgba_sha256']==st['transparent']['rgba_sha256']
original=[]
for e in witness['outline']:
 if e['type']==0:original.append([e['xy']])
 else:assert e['type']==1 and original;original[-1].append(e['xy'])
actual=[]
for c in heavy['actual_local_outlines']:
 points=[c['start']]
 for s in c['segments']:assert s[0]=='line';points.append(s[1])
 actual.append(points)
def fixed(point):
 def qround(v):return math.floor(v+.5)if v>=0 else math.ceil(v-.5)
 return(qround((point[0]*matrix[0]+matrix[6])*64),qround((point[1]*matrix[4]+matrix[7])*64))
def edges(contours):
 counter=collections.Counter();zeros=0
 for contour in contours:
  points=list(map(fixed,contour))
  for a,b in zip(points,points[1:]+points[:1]):
   if a==b:zeros+=1
   else:counter[(a,b)]+=1
 return counter,zeros
sa,sz=edges(original);na,nz=edges(actual)
rows=[]
for i,(a,b)in enumerate(zip(original,actual)):
 mismatch=[{'index':j,'source':x,'native':y,'source_fixed':fixed(x),'native_fixed':fixed(y)}for j,(x,y)in enumerate(zip(a,b))if x!=y]
 if a!=b:rows.append({'contour':i,'source_points':len(a),'native_points':len(b),'raw_different':len(mismatch),'fixed_different':sum(fixed(x)!=fixed(y)for x,y in zip(a,b)),'examples':mismatch[:8]})
def missing(counter):return[{'edge':edge,'count':count}for edge,count in list(counter.items())[:24]]
result={'case':'llamadas-default','source_contours':len(original),'native_contours':len(actual),'raw_boundary_equal':original==actual,'directed_26_6_edges_equal':sa==na,'source_zero_fixed_edges':sz,'native_zero_fixed_edges':nz,'source_only_edge_count':sum((sa-na).values()),'native_only_edge_count':sum((na-sa).values()),'source_only_examples':missing(sa-na),'native_only_examples':missing(na-sa),'changed_contours':rows,'scope':'Exact raw local boundary and actual-affine26.6 directed edges. Zeroedges reported separately, no geometry/winding normalization, pixel fitting or tolerance. Full mask equality is required before using alternate Qt witness.'}
out.parent.mkdir(parents=True,exist_ok=True);out.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items()if not k.endswith('examples')and k!='changed_contours'}))
