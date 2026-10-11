#!/usr/bin/env python3
"""Pinned operative Viewport/SelectTool box selection; no renderer or source edits."""
import os,sys,json,math
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from core.scene import Scene
from core.group import Group
from core.mesh import Mesh
from core.guide import Guide
from core.dimension import Dimension
from core.textlabel import TextLabel
from core.layers import Layer,DEFAULT_LAYER
from tools.select import SelectTool
from views.viewport import Viewport
from formats import igz

def main():
 out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
 app=QApplication.instance() or QApplication([]);vp=Viewport();vp.resize(800,600)
 s=vp.scene;s.layers.append(Layer("named"))
 s.mesh.add_face([V(-3,-1,0),V(-1,-1,0),V(-1,1,0),V(-3,1,0)],[[V(-2.7,-.3,0),V(-2.3,-.3,0),V(-2.3,.3,0),V(-2.7,.3,0)]])
 f=s.mesh.add_face([V(0,-1,0),V(2,-1,0),V(2,1,0),V(0,1,0)]);f.attrs['layer']='named'
 s.mesh.add_edge(V(-5,2,0),V(5,2,0))
 m=Mesh();m.add_face([V(0,0,0),V(1,0,0),V(1,1,0),V(0,1,0)])
 child=Group(m,"child");child.xform=QMatrix4x4();child.xform.translate(3,-2,0);child.xform.rotate(30,0,0,1)
 root=Group(Mesh(),"parent");root.children=[child];root.xform=QMatrix4x4();root.xform.scale(-1,1,1)
 s.groups=[root]
 s.guides=[Guide(V(3,2,0)),Guide(V(0,3,0),V(1,0,0))]
 s.dimensions=[Dimension(V(-1,-3,0),V(2,-3,0),V(0,.5,0),'')]
 s.text_labels=[TextLabel(V(2,3,0),V(1,-1,0),'label',layer='named')]
 vp.camera.target=V(0,0,0);vp.camera.distance=10;vp.camera.yaw=0;vp.camera.pitch=math.pi/2;vp.camera.perspective=False
 vp.camera.set_aspect(800,600)
 igz.save_scene(s,out/'input.igz')
 tool=SelectTool()
 arrays=[('edge',s.edges),('face',s.faces),('group',s.groups),('guide',s.guides),('dimension',s.dimensions),('text',s.text_labels)]
 def selected():return sorted([kind,i]for kind,items in arrays for i,e in enumerate(items) if e in s.selection)
 cases=[]
 rectangles=[('all',[20,20,780,580]),('middle',[350,240,450,360]),('band',[20,270,780,330]),('outside',[-30,-30,-10,-10])]
 for name,rect in rectangles:
  for crossing in [False,True]:
   for mode in ['replace','add','toggle','remove']:
    s.selection={s.faces[0]};before=s.content_version
    tool.on_box_select(vp,rect,crossing,mode=mode)
    assert s.content_version==before
    cases.append(dict(name=f'{name}-{crossing}-{mode}',rect=rect,crossing=crossing,mode=mode,initial=[['face',0]],selected=selected()))
 for policy in ['locked','hidden','restore']:
  layer=s.layers[-1];layer.locked=policy=='locked';layer.visible=policy!='hidden';s.selection=set()
  tool.on_box_select(vp,[20,20,780,580],True,mode='replace')
  cases.append(dict(name=policy,rect=[20,20,780,580],crossing=True,mode='replace',initial=[],policy=policy,selected=selected()))
 for crossing in [False,True]:
  s.selection=set();tool.on_box_select(vp,[-200,-200,1000,800],crossing,mode='replace')
  cases.append(dict(name=f'full-group-{crossing}',rect=[-200,-200,1000,800],crossing=crossing,mode='replace',initial=[],selected=selected()))
 s.begin_group_edit(root)
 arrays=[('edge',s.edges),('face',s.faces),('group',root.children),('guide',s.guides),('dimension',s.dimensions),('text',s.text_labels)]
 for crossing in [False,True]:
  s.selection=set();tool.on_box_select(vp,[-200,-200,1000,800],crossing,mode='replace')
  cases.append(dict(name=f'context-{crossing}',rect=[-200,-200,1000,800],crossing=crossing,mode='replace',initial=[],context=True,selected=selected()))
 payload=dict(width=800,height=600,matrix=list((vp.camera.projection_matrix()*vp.camera.view_matrix()).data()),cases=cases)
 (out/'original.json').write_text(json.dumps(payload,indent=2)+'\n');print(json.dumps({'states':len(cases),'entity_kinds':[k for k,_ in arrays]}))
if __name__=='__main__':main()
