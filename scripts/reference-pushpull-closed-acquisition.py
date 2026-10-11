# SPDX-License-Identifier: GPL-3.0-or-later
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from core.scene import Scene
from core.history import History
from core.orient import is_closed,signed_volume
from tests.test_pushpull_ux import _cube,_top,_push,_StubViewport,_ctx
from tests.test_pushpull_slanted import _gable_house,_roof
from tests.test_fuzz_engine import _draw_rect
from tools.pushpull import PushPullTool
app=QApplication.instance()or QApplication([])
def vec(v):return [v.x(),v.y(),v.z()]
def snap(m):return [dict(vertices=[vec(v)for v in f.vertices],holes=[[vec(v)for v in h]for h in f.holes],attrs=dict(f.attrs),interior=bool(f.interior))for f in m.faces]
rows=[]
for name in ['reversed-cube-cap','reversed-gable-roof','reversed-cube-two-sided','annulus-flush-neighbour','partition-cap-inward']:
 if name=='reversed-gable-roof':s,h=_gable_house();f=_roof(s);d=.3
 else:
  s=Scene();h=History(s);_cube(s,h);f=_top(s,3);d=.5
  if name=='partition-cap-inward':
   _push(s,f,1,keep_base=True);f=_top(s,4);d=-4
  if name=='annulus-flush-neighbour':
   _draw_rect(s,h,[V(1,1,3),V(3,1,3),V(3,3,3),V(1,3,3)],[])
   inner=next(f for f in s.mesh.faces if len(f.vertices)==4 and not f.holes and abs(f.centroid().z()-3)<1e-5 and f.area()<5)
   _push(s,inner,.5)
   f=next(f for f in s.mesh.faces if f.holes and abs(f.centroid().z()-3)<1e-5)
 if name.startswith('reversed'):
  f.loop.reverse()
  for hole in f.hole_loops:hole.reverse()
 f.attrs.update({'color':[.3,.5,.7],'vendor':71})
 if name=='reversed-cube-two-sided':f.attrs['back']={'color':[.8,.2,.1],'opacity':.7}
 before=snap(s.mesh);fid=s.mesh.faces.index(f)
 vp=_StubViewport(s,pick=f);t=PushPullTool();t.hovered_face=f;t.on_click(_ctx(vp));acquired=snap(s.mesh);normal=vec(t._normal)
 t.on_cancel(vp);cancel=snap(s.mesh);cancel_undo=len(vp.history._undo) if hasattr(vp.history,'_undo')else None
 t.hovered_face=f;t.on_click(_ctx(vp));t.extrusion=d;t._commit(vp);after=snap(s.mesh);vp.history.undo();undo=snap(s.mesh);vp.history.redo();redo=snap(s.mesh)
 rows.append(dict(name=name,source=before,face=fid,normal=normal,acquired=acquired,cancel=cancel,undo=undo,redo=redo,requested=d,distance=PushPullTool.last_distance,status=vp.last_status,result=after,closed=is_closed(s.mesh),volume=signed_volume(s.mesh),error=vp.history.last_error,top_out=bool(t._topped_out)))
Path(sys.argv[1]).write_text(json.dumps(dict(source_pin='6be29fe437a7cd992d4e079f71720821b25216b9',cases=rows),indent=2)+'\n')
