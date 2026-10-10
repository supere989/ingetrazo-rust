#!/usr/bin/env python3
"""Actual pinned source paper clipping/closed-loop controls, independent of PDF writer."""
import sys,json,os,subprocess
from pathlib import Path
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtGui import QVector3D as V
from core.section import SectionPlane
from core.hlr import clip_to_section,section_loops
cuts=[]
def ring(points):return list(zip(points,points[1:]+points[:1]))
outer=[[0.,0.,0.],[4.,0.,0.],[4.,4.,0.],[0.,4.,0.]]
inner=[[1.,1.,0.],[1.,3.,0.],[3.,3.,0.],[3.,1.,0.]]
recipes={"ring":ring(outer),"hole":ring(outer)+ring(inner),"open":ring(outer)[:-1],"branch":ring(outer)+[(outer[0],[-2.,0.,0.])],"near_cell":ring([[0.,0.,0.],[1.,0.,0.],[1.,1.,0.],[0.,1.,0.]])+[([0.,1.+2e-8,0.],[0.,2.,0.])],"large_cell":ring([[1e15,1e15,0.],[1e15+1.,1e15,0.],[1e15+1.,1e15+1.,0.],[1e15,1e15+1.,0.]]),"reversed":list(reversed([(b,a)for a,b in ring(outer)]))}
loops=[{"name":name,"cuts":segments,"expected":[r.tolist()for r in section_loops(segments)]}for name,segments in recipes.items()]
clips=[]
for name,normal,triangles in [
 ("axis",[0,1,0],[[[-1.,-1.,0.],[1.,1.,0.],[0.,-1.,2.]]]),
 ("near",[0,1,0],[[[0.,1e-10,0.],[1.,1e-8,0.],[0.,-1e-8,1.]]]),
 ("touch",[0,1,0],[[[0.,0.,0.],[1.,1.,0.],[0.,1.,1.]]]),
 ("skew",[1,2,3],[[[-1.,-2.,3.],[4.,-1.,-2.],[0.,3.,1.]]])]:
 plane=SectionPlane(V(0,0,0),V(*normal));kept,hard,soft,chords=clip_to_section(triangles,[],[],plane,True)
 clips.append({"name":name,"point":[plane.point.x(),plane.point.y(),plane.point.z()],"normal":[plane.normal.x(),plane.normal.y(),plane.normal.z()],"triangles":triangles,"expected_triangles":kept,"expected_cuts":chords})
from core.triangulate import plane_axes
axes=[]
for normal in [[0.,1.,0.],[1.,2.,3.],[1.,0.,0.],[1.,.001,.002],[.123,.567,.321],[1e-8,0.,0.],[1e20,-2e20,3e20]]:
 n=V(*normal);u,v=plane_axes(n)
 axes.append({"normal":[n.x(),n.y(),n.z()],"expected":[[u.x(),u.y(),u.z()],[v.x(),v.y(),v.z()]]})
Path(sys.argv[1]).write_text(json.dumps({"original":subprocess.check_output(["git","-C","ingetrazo","rev-parse","HEAD"],text=True).strip(),"loops":loops,"clips":clips,"axes":axes},indent=2)+"\n")
