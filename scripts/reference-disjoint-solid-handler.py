#!/usr/bin/env python3
"""Only the two missing disjoint outcomes, actual pinned SolidTool._execute."""
import os,sys,json,hashlib,subprocess,tempfile
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');source=Path('/work/ingetrazo');sys.path.insert(0,str(source))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D
from core.group import Group
from core.mesh import Mesh
from core.scene import Scene
from core.history import History
from core import solids
from formats import igz
from tools.solid_tools import SubtractTool,IntersectTool
app=QApplication([])
def cube(x,name):
 m=Mesh()
 v=[QVector3D(x+a,b,c) for a,b,c in [(0,0,0),(1,0,0),(1,1,0),(0,1,0),(0,0,1),(1,0,1),(1,1,1),(0,1,1)]]
 for ids in [(0,3,2,1),(4,5,6,7),(0,1,5,4),(1,2,6,5),(2,3,7,6),(3,0,4,7)]:
  f=m.add_face([v[i] for i in ids]);f.attrs.update({'owner':name,'future':{'opaque':[1,2]}})
 return Group(m)
class Viewport:
 def __init__(self,s):self.scene=s;self.history=History(s);self.flashes=[];self.updates=0
 def flash_status(self,text,ms):self.flashes.append([text,ms])
 def update(self):self.updates+=1
rows=[]
with tempfile.TemporaryDirectory(prefix='disjoint-solid-original-') as d:
 for op,tool in [('subtract',SubtractTool()),('intersect',IntersectTool())]:
  s=Scene();s.groups=[cube(0,'a'),cube(3,'b')];s.selection=set(s.groups);v=Viewport(s);before=Path(d)/'before.igz';after=Path(d)/'after.igz';igz.save_scene(s,before);version=s.version;identities=[id(g) for g in s.groups]
  try:solids.run(op,s.groups)
  except solids.SolidError as e:kernel_error=str(e)
  else:raise AssertionError('disjoint unexpectedly accepted')
  result=tool._execute(v,s.groups);igz.save_scene(s,after)
  assert result==[] and len(v.flashes)==1 and v.updates==0 and not v.history.undo_stack and not v.history.redo_stack and s.version==version and identities==[id(g) for g in s.groups] and before.read_bytes()==after.read_bytes()
  rows.append({'operation':op,'kernel_error':kernel_error,'created':0,'flash':v.flashes[0],'update_calls':v.updates,'undo':0,'redo':0,'version_unchanged':True,'identity_unchanged':True,'scene_bytes_unchanged':True,'scene_sha256':hashlib.sha256(before.read_bytes()).hexdigest()})
r={'source_pin':subprocess.check_output(['git','-C',str(source),'rev-parse','HEAD'],text=True).strip(),'source_sha256':{n:hashlib.sha256((source/n).read_bytes()).hexdigest() for n in ['core/solids.py','tools/solid_tools.py']},'rows':rows,'scope':'actual operation+SolidTool handler; viewport notification sink only, no desktop capture'}
print(json.dumps(r,indent=2))
