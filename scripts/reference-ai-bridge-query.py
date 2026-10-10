#!/usr/bin/env python3
"""Actual frozen bridge query handler, native IGZ inputs and operative edit entry."""
import json,os,sys
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from core.scene import Scene
from core.mesh import Mesh
from core.history import History,AddEdgeCommand
from core.group import Group
from core.layers import Layer
from formats.igz import save_scene,load_into
from plugins.ai_bridge import _Bridge
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
def mesh(offset=0):
 m=Mesh();m.add_face([V(offset,0,0),V(offset+2,0,0),V(offset+2,1,0),V(offset,1,0)]);return m
def group(name,offset=0,placed=False):
 g=Group(mesh(offset),name=name)
 if placed:g.xform=M();g.xform.translate(V(4,2,1))
 return g
def query(s):return _Bridge._tool_query_model(SimpleNamespace(_viewport=SimpleNamespace(scene=s)))
cases=[]
def record(name,s,steps=()):
 path=out/(name+".igz");save_scene(s,path);s=Scene();load_into(s,path);h=History(s);states=[query(s)];history=[]
 for step in steps:
  if step=="enter-root":s.begin_group_edit(s.groups[0])
  elif step=="enter-child":s.begin_group_edit(s.edit_group.children[0])
  elif step=="leave":s.end_one_group_edit()
  elif step=="add-edge":h.execute(AddEdgeCommand(V(101,102,103),V(104,105,106)))
  elif step=="undo":history.append({"tool":step,"result":_Bridge._tool_undo(SimpleNamespace(_viewport=SimpleNamespace(history=h,update=lambda:None)))})
  elif step=="redo":history.append({"tool":step,"result":_Bridge._tool_redo(SimpleNamespace(_viewport=SimpleNamespace(history=h,update=lambda:None)))})
  states.append(query(s))
 cases.append({"name":name,"steps":list(steps),"states":states,"history":history})
record("empty",Scene())
s=Scene();s.mesh.add_edge(V(-3,2,1),V(5,-1,2)).hidden=True;s.mesh.add_face([V(0,0,0),V(2,0,0),V(2,2,0)],hole_loops=[[V(30,30,30),V(31,30,30),V(30,31,30)]]);record("loose-hidden-wire-and-hole",s)
s=Scene();s.layers.append(Layer("off",visible=False));e=s.mesh.add_edge(V(-40,0,0),V(-30,0,0));e.layer="off";e=s.mesh.add_edge(V(1,2,3),V(2,3,4));e.layer="off";s.mesh.add_edge(V(7,8,9),V(8,9,10));record("loose-attribute-visibility",s)
s=Scene();s.groups=[group("Classic",0),group("Placed",10,True)];s.groups[1].hidden=True;record("classic-and-hidden-placement",s)
s=Scene();parent=group("Outer",0,True);child=group("Hidden child",10,True);child.hidden=True;child.children=[group("Visible grandchild",20,True)];parent.children=[child];s.groups=[parent];record("hidden-nested-parent",s)
s=Scene();s.mesh.add_edge(V(-9,-8,-7),V(-6,-5,-4));s.groups=[group("Editable leaf",0,True),group("Sibling",20,True)];record("leaf-context",s,("enter-root","leave"))
s=Scene();s.mesh.add_edge(V(-9,-8,-7),V(-6,-5,-4));parent=group("Editable outer",0,True);parent.children=[group("Editable child",10,True),group("Other child",20,True)];s.groups=[parent,group("Other root",30,True)];record("nested-context",s,("enter-root","enter-child","leave","leave"))
s=Scene();g=group("Fractional",0,True);g.xform.translate(V(.1,.2,.3));g.xform.rotate(35,V(0,0,1));c=group("Fractional child",10,True);c.xform.translate(V(.3,.7,.9));c.xform.rotate(17,V(1,0,0));g.children=[c];s.groups=[g];record("fractional-placement",s)
s=Scene();record("root-history",s,("add-edge","undo","redo","undo","undo","redo"))
s=Scene();s.groups=[group("History leaf",0,True)];record("leaf-history",s,("enter-root","add-edge","undo","redo","leave"))
print(json.dumps({"source_commit":"6be29fe437a7cd992d4e079f71720821b25216b9","cases":cases},indent=2))
