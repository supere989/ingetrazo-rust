#!/usr/bin/env python3
"""Missing invalid UV syntax control only; accepted material corpus untouched."""
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import obj
app=QApplication([]);root=Path(sys.argv[1]);rows=[]
for name,token in [('noninteger','nope'),('out-of-range','99'),('missing','')]:
 path=root/('uv-syntax-'+name+'.obj')
 path.write_text('mtllib uv-syntax.mtl\nv 0 0 0\nv 2 0 0\nv 0 1 0\nvt 0 0\nusemtl tex\nf 1/'+token+' 2/1 3/1\n')
 (root/'uv-syntax.mtl').write_text('newmtl tex\nmap_Kd paint with spaces.png\n')
 scene=Scene();obj.load_obj(scene,path)
 rows.append({'case':name,'faces':len(scene.mesh.faces),'textured':any('texture'in f.attrs for f in scene.mesh.faces),'affine':any('uvw'in f.attrs.get('texture',{})for f in scene.mesh.faces)})
(root/'original-uv-syntax.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps(rows))
