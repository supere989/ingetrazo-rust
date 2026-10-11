#!/usr/bin/env python3
"""Reuse authenticated 28-field corpus, acquire only missing nested records."""
import sys,json,zipfile,hashlib,dataclasses,importlib.util,subprocess
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
spec=importlib.util.spec_from_file_location('prior',Path(__file__).with_name('reference-igz-complete-fields.py'));prior=importlib.util.module_from_spec(spec);spec.loader.exec_module(prior)
from core.scene import Scene
from core import composition as c
from PySide6.QtGui import QVector3D as V
from formats.igz import load_into,save_scene
root=Path(sys.argv[2]);root.mkdir(parents=True,exist_ok=True)
if sys.argv[1]=='prepare':
 audit=Path(__file__).resolve().parents[1]/'docs/audits/igz-complete-fields';m=json.loads((audit/'manifest.json').read_text());raw=(audit/'records.zip').read_bytes();assert hashlib.sha256(raw).hexdigest()==m['archive_sha256']
 with zipfile.ZipFile(audit/'records.zip') as z:
  b=z.read('all-fields.igz');assert hashlib.sha256(b).hexdigest()==m['records']['all-fields.igz'];(root/'retained.igz').write_bytes(b)
 s=Scene();load_into(s,root/'retained.igz');sheet=s.compositions[0]
 inventory={}
 pairs=[('frames',c.MarcoVista),('texts',c.TextoItem),('images',c.ImagenItem),('scalebars',c.BarraEscala),('nortes',c.FlechaNorte),('leyendas',c.Leyenda),('shapes',c.FormaItem),('cotas',c.CotaItem),('cotas_ang',c.CotaAngularItem),('cotas_rad',c.CotaRadialItem),('etiquetas',c.EtiquetaItem),('perfiles',c.PerfilTerreno),('niveles',c.NivelItem),('llamadas',c.LlamadaItem),('cajetin',c.Cajetin)]
 for key,cls in pairs:
  item=cls()
  for attr,value in [('z',7.25),('hidden',True),('locked',True),('group_id','group Ω'),('list_name','Leaf 日本'),('text','Leaf Ω')]:
   if hasattr(item,attr):setattr(item,attr,value)
  if key=='texts':item.italic=True;item.underline=True;item.family='DejaVu Sans';item.bg_color='#123456';item.bg_opacity=.375;item.frame_uid=sheet.frames[0].uid if sheet.frames else ''
  if key=='cajetin':item.campos=[['AUTOR','Author 日本'],['CUSTOM','opaque value']];sheet.cajetin=item
  else:setattr(sheet,key,[item])
  inventory[key]=sorted(dataclasses.asdict(item))
 # Real nested geometry and both supported back-side texture forms.
 face=s.mesh.add_face([V(20,0,0),V(24,0,0),V(24,4,0),V(20,4,0)],[[V(21,1,0),V(21,2,0),V(22,2,0),V(22,1,0)]])
 face.attrs.update({'back':{'texture':dict(s.mesh.faces[0].attrs['texture']),'color':[.3,.2,.1],'opacity':.625},'ifc':{'class':'IfcSlab','custom':{'nested':[1,'Ω']}}})
 mirror=s.mesh.add_face([V(30,0,0),V(31,0,0),V(30,1,0)]);mirror.attrs['back']=True
 save_scene(s,root/'nested-leaves.igz')
 expected={}
 for fixture in ['retained','nested-leaves']:
  for phase in ['edited','undo','redo']:
   q=Scene();load_into(q,root/(fixture+'.igz'))
   if phase!='undo':q.groups.pop()
   f=root/(fixture+'-'+phase+'.expected.igz');save_scene(q,f);expected[fixture+'-'+phase]=prior.observation(f,root/(fixture+'-'+phase+'.source-reopen.igz'))
 (root/'expected.json').write_text(json.dumps({'source_pin':prior.PIN,'leaf_inventory':inventory,'cases':expected},indent=2)+'\n')
else:
 expected=json.loads((root/'expected.json').read_text());observed={k:prior.observation(root/(k+'.native.igz'),root/(k+'.python-reopen.igz')) for k in expected['cases']}
 (root/'observed.json').write_text(json.dumps(observed,indent=2)+'\n')
 def first(a,b,p=''):
  if type(a)!=type(b):return [p,'type',type(a).__name__,type(b).__name__]
  if isinstance(a,dict):
   if set(a)!=set(b):return [p,'keys',list(set(a)-set(b)),list(set(b)-set(a))]
   for k in a:
    d=first(a[k],b[k],p+'/'+k)
    if d:return d
  elif isinstance(a,list):
   if len(a)!=len(b):return [p,'len',len(a),len(b)]
   for i,(x,y) in enumerate(zip(a,b)):
    d=first(x,y,p+'/'+str(i))
    if d:return d
  elif a!=b:return [p,a,b]
 differences={k:first(v,observed[k]) for k,v in expected['cases'].items()};(root/'differences.json').write_text(json.dumps(differences,indent=2)+'\n');print(json.dumps(differences));assert not any(differences.values())
