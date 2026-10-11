"""Focused pinned PlaceGroup controller geometry/ownership oracle."""
import sys,json
sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtGui import QVector3D,QMatrix4x4
from core.mesh import Mesh
from core.group import Group
from tools.place_group import PlaceGroupTool
from types import SimpleNamespace
class View:
 def __init__(self): self.group=None;self.history=SimpleNamespace(execute=self.insert)
 def insert(self,c): self.group=c.group
 def flash_status(self,s): pass
 def window(self): return SimpleNamespace()
 def update(self): pass
def candidate():
 m=Mesh();m.add_edge(QVector3D(0,0,0),QVector3D(2,0,2));return Group(m,'sign')
def point(v):return [v.x(),v.y(),v.z()]
rows=[]
for name,normal,instance in [('plain',None,False),('slab',[0,0,1],False),('wall',[0,1,0],False),('instance',None,True)]:
 g=candidate()
 if instance:
  g.xform=QMatrix4x4();g.xform.translate(5,0,0)
 t=PlaceGroupTool(g,normal is not None);anchor=point(t._anchor)
 if normal:t._face_normal=QVector3D(*normal)
 v=View();t.place_at(v,QVector3D(10,20,30))
 rows.append(dict(name=name,anchor=anchor,vertices=[point(x.position) for x in g.mesh.vertices],matrix=list(g.xform.data()) if instance else None,remaining=t._group is not None))
g=candidate();t=PlaceGroupTool(g);v=View();t.on_cancel(v);rows.append(dict(name='cancel',inserted=v.group is not None,remaining=t._group is not None))
print(json.dumps(rows,indent=2))
