# SPDX-License-Identifier: GPL-3.0-or-later
# Reuse frozen geometry recipes; capture a different operative caller (on_value).
import os,sys,json,hashlib
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from core.scene import Scene
from tests.test_pushpull_ux import _StubViewport,_ctx
from tools.pushpull import PushPullTool
app=QApplication.instance()or QApplication([])
def vec(v):return [v.x(),v.y(),v.z()]
def snap(m):return [dict(vertices=[vec(v)for v in f.vertices],holes=[[vec(v)for v in h]for h in f.holes],attrs=dict(f.attrs),interior=bool(f.interior))for f in m.faces]
original=Path(sys.argv[1]).read_bytes();input=json.loads(original);rows=[]
for recipe in input['cases']:
 s=Scene()
 for raw in recipe['source']:
  f=s.mesh.add_face([V(*p)for p in raw['vertices']],hole_loops=[[V(*p)for p in h]for h in raw['holes']]);f.attrs=dict(raw['attrs']);f.interior=raw['interior']
 assert snap(s.mesh)==recipe['source']
 f=s.mesh.faces[recipe['face']];vp=_StubViewport(s,pick=f);t=PushPullTool();t.hovered_face=f;t.on_click(_ctx(vp));acquired=snap(s.mesh);normal=vec(t._normal);limit=t._limit_in;t.on_cancel(vp);cancel=snap(s.mesh)
 t.hovered_face=f;t.on_click(_ctx(vp));t.extrusion=recipe['requested'];vcb=abs(recipe['requested']);assert t.on_value(vp,vcb);result=snap(s.mesh);distance=PushPullTool.last_distance;vp.history.undo();undo=snap(s.mesh);vp.history.redo();redo=snap(s.mesh)
 rows.append(dict(name=recipe['name'],source=recipe['source'],face=recipe['face'],requested=recipe['requested'],vcb_positive_magnitude=vcb,normal=normal,limit=limit,distance=distance,acquired=acquired,cancel=cancel,result=result,undo=undo,redo=redo,status=vp.last_status,error=vp.history.last_error))
Path(sys.argv[2]).write_text(json.dumps(dict(source_pin=input['source_pin'],retained_input_sha256=hashlib.sha256(original).hexdigest(),caller='Actual on_click/on_cancel/on_value positive magnitude during signed drag, History Undo/Redo',cases=rows),indent=2)+'\n')
