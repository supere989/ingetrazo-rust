#!/usr/bin/env python3
"""Compare single-edge rounded geometry to the original fillet engine."""
import json,os,sys,subprocess,tempfile
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from core.group import Group
from core import solids
from core.fillet import plan_fillet,apply_fillet
from formats import igz

def manifold(mesh):return solids._to_manifold(Group(mesh),solids._Sources())
def main():
    app=QApplication.instance() or QApplication([]);rows=[]
    with tempfile.TemporaryDirectory() as temp:
        subprocess.run(["cargo","run","--locked","-p","ingetrazo-tools","--example","fillet_acceptance_fixture","--",temp],check=True)
        for name,radius,segments in [("r02-s8",.2,8),("r01-s3",.1,3),("r03-s16",.3,16),("rim",.2,8),("all",.2,8)]:
            original=Scene();igz.load_into(original,Path(temp)/(name+"-source.igz"));unchanged=manifold(original.mesh)
            edge=next(e for e in original.mesh.edges if e.a.y()==0 and e.b.y()==0 and e.a.z()==1 and e.b.z()==1)
            edges=list(original.mesh.edges) if name=="all" else [e for e in original.mesh.edges if e.a.z()==1 and e.b.z()==1] if name=="rim" else [edge]
            plan=plan_fillet(original.mesh,edges,radius,segments);assert not isinstance(plan,str),plan;apply_fillet(original.mesh,plan)
            rust=Scene();igz.load_into(rust,Path(temp)/(name+"-result.igz"));a,b=manifold(original.mesh),manifold(rust.mesh)
            difference=(a-b).volume()+(b-a).volume();assert difference<1e-6,(name,difference)
            assert (unchanged-b).volume()>1e-3,"Softening an unchanged edge must fail the geometric oracle"
            rows.append({"case":name,"radius":radius,"segments":segments,"original_volume":a.volume(),"rust_volume":b.volume(),"symmetric_difference_volume":difference})
    report={"provenance":context(),"passed":True,"negative_controls_detected":len(rows),"cases":rows,"scope":"Single convex box edge at three radii/segment counts, four-edge top rim and all twelve box edges with spherical joins. Independent original/native manifold shape oracle. General concave/mixed selections and interactive acceptance remain open."}
    Path("docs/audits/python-fillet-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
