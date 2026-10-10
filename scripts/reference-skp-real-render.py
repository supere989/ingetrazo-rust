#!/usr/bin/env python3
"""Actual frozen original import versus native IGZ through the same Qt renderer."""
import sys,json,time,math,hashlib,subprocess
from pathlib import Path
root=Path(__file__).resolve().parents[1];oracle=Path(sys.argv[1]);native=Path(sys.argv[2]);out=Path(sys.argv[3]);out.mkdir(parents=True,exist_ok=True)
assert subprocess.check_output(['git','-C',str(oracle),'rev-parse','HEAD'],text=True).strip()=='291700bc213655a00461838de71e5f206311e619'
assert subprocess.check_output(['git','-C',str(root/'ingetrazo'),'rev-parse','HEAD'],text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9'
sys.path[:0]=[str(oracle/'packages/python/src'),str(root/'ingetrazo')]
from PySide6.QtGui import QSurfaceFormat,QVector3D,QMatrix4x4
from PySide6.QtWidgets import QApplication
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile);fmt.setDepthBufferSize(24);fmt.setStencilBufferSize(8);fmt.setSamples(4);QSurfaceFormat.setDefaultFormat(fmt)
app=QApplication([])
from core.scene import Scene
from core.style import Style
from formats.skp import load_skp
from formats.igz import load_into
from views.viewport import Viewport
# Inventory comparison remains mandatory beside pixel captures.
s=(root/'scripts/check-native-skp-vff.py').read_text();exec(s[s.index('def ring('):s.index('rows=[]')])
s=(root/'scripts/check-native-skp-legacy-model.py').read_text();exec(s[s.index('def inventory'):s.index('rows=[]\nfor name')])
rows=[];protocol=[]
for name in ['gondola_v20','capilla_quiroz_v17']:
 path=oracle/'packages/python/tests/fixtures'/(name+'.skp');igz=out/(name+'.igz');igz.unlink(missing_ok=True)
 result=subprocess.run([str(native),str(path),str(igz),str(out/(name+'-payload.json'))],capture_output=True,text=True,timeout=30);(out/(name+'-native.log')).write_text(result.stdout+result.stderr);assert result.returncode==0,result.stderr
 original=Scene();load_skp(original,path);reopened=Scene();load_into(reopened,igz)
 a=scene(original);b=scene(reopened);a['inventory']=inventory(original);b['inventory']=inventory(reopened);assert a==b;import gzip
 (out/(name+'-inventory.json.gz')).write_bytes(gzip.compress(json.dumps({'original':a,'native':b}).encode(),mtime=0))
 points=[p for g in a['groups']for ring in g['world']for p in ring];assert points
 lo=[min(p[k]for p in points)for k in range(3)];hi=[max(p[k]for p in points)for k in range(3)]
 # Freeze camera before painting; no post-capture fitting/alignment.
 target=QVector3D(*[(lo[k]+hi[k])/2 for k in range(3)]).toTuple();distance=math.sqrt(sum((hi[k]-lo[k])**2 for k in range(3)))*1.5;assert distance>0
 for view,yaw,pitch in [('iso',-45.,30.),('asymmetric',28.,24.)]:
  camera={'target':target,'distance':distance,'yaw':math.radians(yaw),'pitch':math.radians(pitch),'fov_deg':45.,'perspective':False,'width':800,'height':600,'background':[1.,1.,1.],'edges':True,'profiles':False,'face_mode':'shaded','textures':False}
  record={'name':name+'-'+view,'model':name,'camera':camera,'input_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'igz_sha256':hashlib.sha256(igz.read_bytes()).hexdigest()};protocol.append(record)
  captures=[]
  for label,model in [('source',original),('native-igz',reopened)]:
   vp=Viewport();vp.resize(800,600);vp.scene=model;vp.style_override=Style(face_mode='shaded',sky=False,edges=True,profiles=False,background=(1.,1.,1.));vp.camera.target=QVector3D(*target);vp.camera.distance=distance;vp.camera.yaw=camera['yaw'];vp.camera.pitch=camera['pitch'];vp.camera.fov_deg=45.;vp.camera.perspective=False;vp.show()
   deadline=time.monotonic()+4
   while time.monotonic()<deadline:app.processEvents();time.sleep(.01)
   image=vp.grabFramebuffer();assert not image.isNull()and image.width()==800 and image.height()==600;filename=out/(record['name']+'-'+label+'.png');assert image.save(str(filename));captures.append(image)
   vp.close();vp.deleteLater();app.processEvents()
  left,right=captures;different=sum(left.pixel(x,y)!=right.pixel(x,y)for y in range(600)for x in range(800));ink=sum(left.pixelColor(x,y).red()<245 or left.pixelColor(x,y).green()<245 or left.pixelColor(x,y).blue()<245 for y in range(600)for x in range(800));assert ink>1000
  rows.append({**record,'different_pixels':different,'source_ink_pixels':ink,'exact':different==0,'source_image_sha256':hashlib.sha256((out/(record['name']+'-source.png')).read_bytes()).hexdigest(),'native_igz_image_sha256':hashlib.sha256((out/(record['name']+'-native-igz.png')).read_bytes()).hexdigest()})
(out/'protocol.json').write_text(json.dumps({'cases':protocol,'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','parser_pin':'291700bc213655a00461838de71e5f206311e619','binary_sha256':hashlib.sha256(native.read_bytes()).hexdigest()},indent=2)+'\n')
(out/'qt-isolation-results.json').write_text(json.dumps({'passed':all(r['exact']for r in rows),'cases':rows,'scope':'Qt renderer import isolation only; native Vulkan stage required'},indent=2)+'\n');assert all(r['exact']for r in rows),'strict pixel failure retained'
