#!/usr/bin/env python3
import argparse,json
from pathlib import Path
from PySide6.QtGui import QImage
BASE=['red-half-white','red-half-black','opacity-bucket-order','opaque-front','opaque-back','equal-opacity-forward','equal-opacity-reversed','opacity-995','opacity-zero','section-clipped']
EDGE=['style-wireframe-asymmetric-line','style-wireframe-reflected-line','style-wireframe-hidden-group','style-wireframe-hidden-edge','style-wireframe-hidden-layer','style-wireframe-soft-line','style-wireframe-clipped-line','style-hidden_line-occluded-line','style-xray-occluded-line']
def check(source,actual,window=False):
 s=json.loads((source/'original-alpha-results.json').read_text())['cases'];n=json.loads((actual/('native-ui-results.json'if window else'native-alpha-results.json')).read_text())['cases']
 assert [r['case']for r in s]==[r['case']for r in n]==BASE+EDGE,'Incomplete, duplicate or reordered corpus'
 for a,b in zip(s,n):
  assert 'NVIDIA'in a['gl_info']['renderer']and not a['gl_info']['software']
  assert a['center_rgba']==b['center_rgba']
  if window:assert b['passed']is True
 rows=[]
 for name in EDGE:
  a,b=QImage(str(source/(name+'.png'))),QImage(str(actual/(name+'.png')))
  assert not a.isNull()and not b.isNull()and a.width()==b.width()==800 and a.height()==b.height()==600
  differences=sum(a.pixel(x,y)!=b.pixel(x,y)for y in range(600)for x in range(800))
  rows.append({'case':name,'differing_rgba_pixels':differences,'passed':differences==0})
 return {'passed':all(r['passed']for r in rows),'cases':rows,'mandatory_metadata_rows':19,'whole_image_cases':9,'rgba_tolerance':0}
def main():
 p=argparse.ArgumentParser();p.add_argument('source');p.add_argument('actual');p.add_argument('output');p.add_argument('--window',action='store_true');a=p.parse_args()
 result=check(Path(a.source),Path(a.actual),a.window);Path(a.output).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));raise SystemExit(0 if result['passed']else 1)
if __name__=='__main__':main()
