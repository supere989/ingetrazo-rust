import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from core.scene import Scene
from tests.test_pushpull_ux import _wall_with_door,_StubViewport,_ctx
from tests.test_pushpull_recess_seen_from_behind import _wall_with_door as behind_wall,_Vp
from tools.pushpull import PushPullTool
app=QApplication.instance()or QApplication([])
def vec(v):return [v.x(),v.y(),v.z()]
def snap(m):return [dict(vertices=[vec(v)for v in f.vertices],holes=[[vec(v)for v in h]for h in f.holes],attrs=dict(f.attrs),interior=bool(f.interior))for f in m.faces]
rows=[]
for name in ['reversed-door','lone-sheet','front-recess','back-recess']:
 s=Scene()
 if name=='reversed-door':_,f=_wall_with_door(s)
 elif name=='lone-sheet':f=s.mesh.add_face([V(0,0,0),V(2,0,0),V(2,2,0),V(0,2,0)])
 else:f=behind_wall(s)
 f.attrs.update({'color':[.3,.5,.7],'vendor':71});before=snap(s.mesh);fid=s.mesh.faces.index(f)
 vp=_StubViewport(s,pick=f);t=PushPullTool();t.hovered_face=f;t.on_click(_ctx(vp));normal=vec(t._normal);d=-.6
 if name=='back-recess':d=.6
 t.extrusion=d
 if name in ['front-recess','back-recess']:
  previewvp=_Vp(s,V(2.5,5 if name=='front-recess'else -5,1));t._show_light_preview(previewvp);suppressed=f in previewvp.suppressed
 else:t._show_light_preview(vp);suppressed=f in (vp.suppressed or set())
 preview=[dict(vertices=[vec(v)for v in face.vertices]) for face in t._light_faces]
 inert=snap(s.mesh)==before;t._commit(vp);after=snap(s.mesh);vp.history.undo();undo=snap(s.mesh)==before;vp.history.redo();redo=snap(s.mesh)==after
 rows.append(dict(name=name,source=before,face=fid,normal=normal,distance=d,suppressed=suppressed,preview=preview,preview_inert=inert,result=after,undo_exact=undo,redo_exact=redo,error=vp.history.last_error))
Path(sys.argv[1]).write_text(json.dumps(dict(source_pin='6be29fe437a7cd992d4e079f71720821b25216b9',cases=rows),indent=2)+'\n')
