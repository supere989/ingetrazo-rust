#!/usr/bin/env python3
"""Replay 45 strict whole images against the frozen original; no tolerance."""
import os,sys,subprocess,json
from pathlib import Path
root=Path(__file__).resolve().parents[3]
binary=Path(sys.argv[1]).resolve()
out=Path(sys.argv[2]).resolve()
out.mkdir(parents=True,exist_ok=False)
rows=[]
for name,zoom,ppp,x,y in [('fractional',8,1,12.25,12.5),('zoom-small',2,1,12.25,12.5),('density125',8,1.25,50,50),('density200',8,2,50,50),('negative-origin',8,1,-12.25,-12.5)]:
 env=dict(os.environ,QT_QPA_PLATFORM='offscreen',INGETRAZO_CAD2D_INTEGER_BLEND='1',INGETRAZO_SHAPE_DIRECT='1',INGETRAZO_SHAPE_ANTIALIAS='1',INGETRAZO_SHAPE_ZOOM=str(zoom),INGETRAZO_SHAPE_PPP=str(ppp),INGETRAZO_SHAPE_ORIGIN_X=str(x),INGETRAZO_SHAPE_ORIGIN_Y=str(y))
 p=out/name;p.mkdir()
 for script,args,log in [('reference-original-sheet-shapes.py',[str(p/'source')],'source.log'),('check-native-sheet-shape-pixels.py',[str(binary),str(p/'source'),str(p/'native')],'native.log')]:
  with (p/log).open('w') as f:
   subprocess.run([sys.executable,str(root/'scripts'/script),*args],cwd=root,env=env,stdout=f,stderr=subprocess.STDOUT,check=True)
 rows.append(dict(name=name,result=json.loads((p/'native/result.json').read_text())))
(out/'matrix.json').write_text(json.dumps(dict(passed=True,whole_images=45,variants=rows),indent=2)+chr(10))
print('PASS 45 strict whole images')
