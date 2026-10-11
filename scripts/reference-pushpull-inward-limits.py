import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as Q
from core.scene import Scene
from core.history import History
from tests.test_pushpull_ux import _cube,_top,_push,_locked_tool,_StubViewport
from tests.test_pushpull_slanted import _wedge,_gable_house,_roof
from tests.test_pushpull_against_wall import _step_with_square
app=QApplication.instance()or QApplication([])
def V(x,y,z=0):return Q(x,y,z)
def p(v):return [v.x(),v.y(),v.z()]
def snap(m):return [{'vertices':[p(v)for v in f.vertices],'holes':[[p(v)for v in h]for h in f.holes],'attrs':dict(f.attrs),'interior':bool(f.interior)}for f in m.faces]
def triangle():
 s=Scene();f=s.mesh.add_face([V(0,0),V(4,0),V(0,4)]);_push(s,f,3);return s
recipes=[]
s=Scene();_cube(s,History(s),height=3);recipes.append(('cube-cap',s,_top(s,3),-99,False))
for name,pred in [('triangle-long-wall',lambda f:f.normal().x()>.1 and f.normal().y()>.1),('triangle-axis-wall',lambda f:f.normal().y()<-.5),('triangle-end',lambda f:f.normal().z()>.5)]:
 s=triangle();recipes.append((name,s,next(f for f in s.mesh.faces if pred(f)),-99,False))
for name,pred in [('wedge-roof',lambda f:f.normal().z()>.5),('wedge-xwall',lambda f:f.normal().x()>.9),('wedge-ywall',lambda f:f.normal().y()<-.9)]:
 s,_=_wedge();recipes.append((name,s,next(f for f in s.mesh.faces if pred(f)),-99,False))
s,_=_gable_house();recipes.append(('gable-roof',s,_roof(s),-99,False))
s,f=_step_with_square();recipes.append(('step-mouth',s,f,-99,False))
s=Scene();f=s.mesh.add_face([V(0,0),V(2,0),V(2,2),V(0,2)]);s.mesh.add_face([V(-1,-1,-1),V(-1,3,-1),V(3,3,-1),V(3,-1,-1)]);recipes.append(('unattached-sheet',s,f,-.25,False))
s=Scene();_cube(s,History(s),height=3);f=_top(s,3);s.mesh.add_face([V(2,-1,2),V(2,5,2),V(5,5,2),V(5,-1,2)]);recipes.append(('partial-parallel',s,f,-99,False))
s=Scene();_cube(s,History(s),height=3);f=_top(s,3);s.mesh.add_face([V(5,0,2),V(5,4,2),V(9,4,2),V(9,0,2)]);recipes.append(('disjoint-parallel',s,f,-99,False))
s=Scene();_cube(s,History(s),height=3);recipes.append(('ctrl-cap',s,_top(s,3),-99,True))
rows=[]
for name,s,f,requested,keep in recipes:
 before=snap(s.mesh);fid=s.mesh.faces.index(f);anchor=p(f.centroid());tool=_locked_tool(s,f,requested);tool._keep_base=keep;vp=_StubViewport(s);tool.extrusion=requested;tool._clamp_extrusion(vp);limit=tool._limit_in;distance=tool.extrusion
 # Commit the actual clamped request. Known source invalidity is retained.
 tool._commit(vp);after=snap(s.mesh)
 from core.orient import is_closed
 rows.append({'name':name,'source':before,'face':fid,'anchor':anchor,'limit':limit,'distance':distance,'requested':requested,'ctrl':keep,'status':vp.last_status,'result':after,'source_closed':is_closed(s.mesh),'source_error':vp.history.last_error,'undo':len(vp.history.undo_stack)})
 # The face index and anchor must describe the pre-commit live face.
Path(sys.argv[1]).write_text(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','cases':rows},indent=2)+'\n')
