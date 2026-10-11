import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from core.scene import Scene
from core.history import History
from tests.test_pushpull_ux import _cube,_top,_locked_tool,_StubViewport
app=QApplication.instance()or QApplication([])
def snap(m):
 def p(v):return [v.x(),v.y(),v.z()]
 return [{'vertices':[p(v)for v in f.vertices],'holes':[[p(v)for v in h]for h in f.holes],'attrs':dict(f.attrs),'interior':bool(f.interior)}for f in m.faces]
rows=[]
for value in [99.,-99.]:
 s=Scene();_cube(s,History(s),height=3);f=_top(s,3);tool=_locked_tool(s,f,-.5);vp=_StubViewport(s);accepted=tool.on_value(vp,value)
 rows.append({'value':value,'accepted':accepted,'status':vp.last_status,'result':snap(s.mesh),'undo':len(vp.history.undo_stack)})
Path(sys.argv[1]).write_text(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','cases':rows},indent=2)+'\n')
