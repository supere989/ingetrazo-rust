#!/usr/bin/env python3
"""Differential shapes: original Python booleans versus Rust, symmetric difference."""
import json,math,os,sys,subprocess,tempfile
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D
from core.group import Group
from core.mesh import Mesh
from core.scene import Scene
from core import solids
from formats import igz

def cube(x,scenario=None):
    vertices=[QVector3D(*p) for p in [(x,0,0),(x+1,0,0),(x+1,1,0),(x,1,0),(x,0,1),(x+1,0,1),(x+1,1,1),(x,1,1)]]
    if scenario=="rotated":
        c,s=math.cos(math.pi/6),math.sin(math.pi/6)
        vertices=[QVector3D(c*p.x()-s*p.y()+.5,s*p.x()+c*p.y()+.2,p.z()) for p in vertices]
    if scenario=="contained":vertices=[p*.5+QVector3D(.25,.25,.25) for p in vertices]
    mesh=Mesh()
    for face in [(0,3,2,1),(4,5,6,7),(0,1,5,4),(1,2,6,5),(2,3,7,6),(3,0,4,7)]:mesh.add_face([vertices[i] for i in face])
    return Group(mesh)
def manifold(group):return solids._to_manifold(group,solids._Sources())
def mismatch(a,b):return (a-b).volume()+(b-a).volume()
def main():
    app=QApplication.instance() or QApplication([])
    rows=[]
    with tempfile.TemporaryDirectory() as temp:
        subprocess.run(["cargo","run","--locked","-p","ingetrazo-formats","--example","boolean_acceptance_fixture","--",temp],check=True)
        for scenario in ("overlap","rotated","contained"):
            for name in ("union","subtract","intersect"):
                a,b=cube(0),cube(.5 if scenario=="overlap" else 0,scenario)
                original=solids.run(name,[b,a] if name=="subtract" else [a,b])[1][0]
                scene=Scene();igz.load_into(scene,Path(temp)/(scenario+"-"+name+".igz"))
                rust=manifold(Group(scene.mesh));reference=manifold(original)
                error=mismatch(rust,reference)
                assert abs(rust.volume()-reference.volume())<1e-6 and error<1e-6,(scenario,name,error,rust.volume())
                rows.append({"scenario":scenario,"operation":name,"original_volume":reference.volume(),"rust_volume":rust.volume(),"symmetric_difference_volume":error})
        # Shape oracle must reject a copied input as intersection result.
        assert mismatch(manifold(cube(0)),reference)>.1
    report={"provenance":context(),"passed":True,"negative_controls_detected":1,"operations":rows,"scope":"Overlapping, rotated and contained boxes; original Python shape oracle via independent native manifold3d. No arbitrary/degenerate/attribute/GUI parity claim."}
    Path("docs/audits/python-boolean-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
