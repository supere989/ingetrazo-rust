#!/usr/bin/env python3
"""Read-only actual source shading inventory; no pixel acceptance or emulated renderer."""
import sys,json,subprocess,hashlib,collections
from pathlib import Path
root=Path(__file__).resolve().parents[1];oracle=Path(sys.argv[1]);out=Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True)
pins={'app':'6be29fe437a7cd992d4e079f71720821b25216b9','parser':'291700bc213655a00461838de71e5f206311e619'}
for key,path in [('app',root/'ingetrazo'),('parser',oracle)]:assert subprocess.check_output(['git','-C',str(path),'rev-parse','HEAD'],text=True).strip()==pins[key]
sys.path[:0]=[str(oracle/'packages/python/src'),str(root/'ingetrazo')]
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats.skp import load_skp
from views.viewport import Viewport
app=QApplication([]);vp=Viewport();records=[]
for name in ['gondola_v20','capilla_quiroz_v17']:
 path=oracle/'packages/python/tests/fixtures'/(name+'.skp');scene=Scene();load_skp(scene,path);vp.scene=scene;rows=[]
 def visit(group,chain):
  for index,face in enumerate(group.mesh.faces):
   attrs=face.attrs;normal=vp._normal_of(face);base=tuple(attrs.get('color',vp.DEFAULT_FACE_COLOR))[:3]
   rows.append({'group':chain,'face':index,'texture':attrs.get('texture') is not None,'normal':normal.toTuple(),'base':base,'source_shade':vp._shade_factor(normal),'source_shaded_rgb':vp._shaded_color(base,normal),'unquantized_rgb':[min(1.,c*vp._shade_factor(normal))for c in base]})
  for index,child in enumerate(group.children):visit(child,chain+[index])
 for index,group in enumerate(scene.groups):visit(group,[index])
 color_counts=collections.Counter(tuple(round(c*255)for c in row['source_shaded_rgb'])for row in rows if not row['texture'])
 record={'name':name,'input_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'face_count':len(rows),'ordinary_face_colors':[{'rgb8':list(rgb),'face_count':count}for rgb,count in color_counts.most_common()],'faces':rows};records.append(record)
(out/'source-shaded-runs.json').write_text(json.dumps({'pins':pins,'scope':'actual source CPU shaded-run law; texture averages and world transforms require separate routing','models':records},indent=2)+'\n');print([(r['name'],r['face_count'])for r in records])
