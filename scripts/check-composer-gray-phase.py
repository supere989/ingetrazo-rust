#!/usr/bin/env python3
"""Actual original/production shape whole images at mandatory phase/density variants."""
import json,os,subprocess,sys
from pathlib import Path
root=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]).resolve();out.mkdir(parents=True,exist_ok=True);rows=[]
variants=[('fractional',8,1,50.25,50.75),('zoom-small',2,1,12.25,12.5),('density125',8,1.25,50.25,50.75),('density200',4,2,25.125,25.375),('negative-origin',8,1,-.25,-.75)]
for name,zoom,ppp,x,y in variants:
 case=out/name;case.mkdir(exist_ok=True);source=case/'original';native=case/'native';env=dict(os.environ,INGETRAZO_SHAPE_ANTIALIAS='1',INGETRAZO_SHAPE_DIRECT='1',INGETRAZO_SHAPE_ZOOM=str(zoom),INGETRAZO_SHAPE_PPP=str(ppp),INGETRAZO_SHAPE_ORIGIN_X=str(x),INGETRAZO_SHAPE_ORIGIN_Y=str(y),QT_QPA_PLATFORM='offscreen')
 with(case/'original.log').open('w')as log:subprocess.run([sys.executable,str(root/'scripts/reference-original-sheet-shapes.py'),str(source)],env=env,cwd=root,stdout=log,stderr=log,check=True)
 nativeenv=dict(env);nativeenv.pop('QT_QPA_PLATFORM',None)
 with(case/'native.log').open('w')as log:run=subprocess.run([sys.executable,str(root/'scripts/check-native-sheet-shape-pixels.py'),str(root/'target/debug/examples/sheet_shape_render_fixture'),str(source),str(native)],env=nativeenv,cwd=root,stdout=log,stderr=log)
 report=json.loads((native/'result.json').read_text())if(native/'result.json').is_file()else{'passed':False,'error':'missing fresh complete native result'};rows.append({'name':name,'exit_code':run.returncode,'passed':report.get('passed')and run.returncode==0,'report':report})
assert [r['name']for r in rows]==[v[0]for v in variants]
report={'passed':all(r['passed']for r in rows),'variants':rows};(out/'verdict.json').write_text(json.dumps(report,indent=2));print(json.dumps(report));assert report['passed']
