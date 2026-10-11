#!/usr/bin/env python3
"""Pinned original BIM observations. Transformed-group export discrepancy explicit."""
import sys,json,hashlib,subprocess
from pathlib import Path
root=Path(__file__).resolve().parents[1];source=root/'ingetrazo';out=Path(sys.argv[1]);out.mkdir(exist_ok=True,parents=True)
pin='6be29fe437a7cd992d4e079f71720821b25216b9';assert subprocess.check_output(['git','-C',str(source),'rev-parse','HEAD'],text=True).strip()==pin
sys.path.insert(0,str(source))
from PySide6.QtGui import QVector3D,QMatrix4x4
from core.mesh import Mesh
from core.scene import Scene
from core.group import Group,world_mesh
from core.bim import IFC_CLASSES,collect_objects,class_quantities,quantities_csv,tag_faces,tag_group
from formats.igz import save_scene
from formats.ifc import save_ifc

def box():
 m=Mesh();p=[QVector3D(x,y,z)for z in [0,4]for y in [0,3]for x in [0,2]]
 for ring in [[0,2,3,1],[4,5,7,6],[0,1,5,4],[1,3,7,5],[3,2,6,7],[2,0,4,6]]:m.add_face([p[i]for i in ring])
 return m

def observe(name,scene):
 objects=[]
 for o in collect_objects(scene):
  g=o.get('group');faces=o.get('faces')
  if faces is None:faces=list(world_mesh(g).faces) if g.xform is not None or g.children else list(g.mesh.faces)
  qset,entries,(met,unit)=class_quantities(o['class'],faces)
  objects.append({'class':o['class'],'name':o['name'],'area':o['area'],'volume':o['volume'],'quantities':{'set':qset,'entries':[{'kind':k,'name':n,'value':v}for k,n,v in entries],'metrado':met,'unit':unit},'rings':[[[list(v.position.toTuple())for v in lp]for lp in [f.loop,*f.hole_loops]]for f in faces]})
 save_scene(scene,out/(name+'.igz'));(out/(name+'-source.csv')).write_text(quantities_csv(scene))
 try:save_ifc(scene,out/(name+'-source.ifc'));failure=None
 except Exception as e:failure=str(e)
 return {'name':name,'objects':objects,'source_export_error':failure}
records=[]
for cls in IFC_CLASSES:
 s=Scene();s.mesh=box();tag_faces(s.mesh.faces,cls,'Muro de José '+cls,1);records.append(observe(cls,s))
s=Scene();f=s.mesh.add_face([QVector3D(0,0,0),QVector3D(4,0,0),QVector3D(4,0,3),QVector3D(0,0,3)],hole_loops=[[QVector3D(1,0,1),QVector3D(1,0,2),QVector3D(2,0,2),QVector3D(2,0,1)]]);tag_faces([f],'IfcWall','Perforado',1);records.append(observe('wall-hole',s))
s=Scene();f=s.mesh.add_face([QVector3D(0,0,0),QVector3D(3,4,0),QVector3D(3,4,2),QVector3D(0,0,2)]);tag_faces([f],'IfcDoor','Rotated',1);records.append(observe('rotated-door',s))
s=Scene();g=Group(box(),'Parent');g.xform=QMatrix4x4();g.xform.translate(10,20,30);g.xform.scale(2,3,4);g.hidden=True;child=Group(box(),'Child');child.xform=QMatrix4x4();child.xform.translate(8,0,0);g.children=[child];tag_group(g,'IfcColumn','Nested hidden');s.groups.append(g);s.mesh=box();records.append(observe('transformed-nested',s))
s=Scene();s.mesh=box();tag_faces(s.mesh.faces,"IfcBogus');#1=IFCWALL(",'Fallback',1);records.append(observe('unknown-class',s))
s=Scene();s.mesh=box();records.append(observe('untagged',s))
files=['core/bim.py','formats/ifc.py','core/group.py','core/mesh.py']
report={'source_pin':pin,'source_files':{f:hashlib.sha256((source/f).read_bytes()).hexdigest()for f in files},'cases':records,'scope':'Actual original collect_objects and class_quantities. For transformed groups class quantities use original world_mesh; original quantities_csv/save_ifc instead use local group mesh, a retained source discrepancy. Native export consistently uses world geometry.'}
(out/'original.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'cases':len(records),'objects':sum(len(r['objects'])for r in records)}))
