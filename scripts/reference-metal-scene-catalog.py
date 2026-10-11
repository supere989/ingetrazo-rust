#!/usr/bin/env python3
"""Actual frozen source placement expansion, top owner paint and matrix recipe."""
import pathlib,sys,subprocess,json
repo=pathlib.Path(__file__).resolve().parents[1];source=repo/'ingetrazo'
pin=subprocess.check_output(['git','-C',str(source),'rev-parse','HEAD'],text=True).strip();assert pin=='6be29fe437a7cd992d4e079f71720821b25216b9'
sys.path.insert(0,str(source))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D,QMatrix4x4
app=QApplication([])
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group,effective_material
from core.materials import material_sig
from views.viewport import Viewport
m=Mesh();m.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])
root=Group(m,'root');root.xform=QMatrix4x4();root.xform.translate(0,2,0);root.material={'color':[1,0,0]}
a=Group(m,'A');b=Group(m,'B');b.xform=QMatrix4x4();b.xform.translate(4,0,0)
mid=Group(Mesh(),'middle');mid.material={'color':[0,1,0]};mid.adopt([a,b]);root.adopt([mid]);s=Scene();s.groups=[root]
vp=Viewport();vp.scene=s
rows=[]
for g in vp._placements():
 if not g.mesh.faces:continue
 paint=effective_material(g);matrix=list(g.xform.data()) if g.xform is not None else None
 rows.append({'name':g.name,'paint':paint,'signature':material_sig(paint),'matrix_columns':matrix,'same_mesh_as_root':g.mesh is m,'owner':getattr(g,'owner',None).name if getattr(g,'owner',None)else None})
assert [r['name'] for r in rows]==['root','A','B']
assert all(r['paint']=={'color':[1,0,0]} for r in rows)
assert rows[1]['matrix_columns'][12:16]==[0,2,0,1]
assert rows[2]['matrix_columns'][12:16]==[4,2,0,1]
assert all(r['same_mesh_as_root'] for r in rows)
# Source signatures ignore opaque vendor metadata and numeric signed zero.
assert material_sig({'color':[0.,1.,0.],'vendor':71})==material_sig({'color':[-0.,1.,0.]})
out=pathlib.Path(sys.argv[1]);out.parent.mkdir(parents=True,exist_ok=True);out.write_text(json.dumps({'passed':True,'source_pin':pin,'scope':'Actual Viewport._placements/core.effective_material metadata; no GL/GPU/UI claim','rows':rows},indent=2)+'\n');print(out)
