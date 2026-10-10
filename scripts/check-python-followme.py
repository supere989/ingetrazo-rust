#!/usr/bin/env python3
"""Original sweep construction versus Rust connected path geometry."""
import json,os,sys,subprocess,tempfile
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D
from core.scene import Scene
from core.group import Group
from core import solids
from core.sweep import sweep_profile
from formats import igz

def manifold(mesh):return solids._to_manifold(Group(mesh),solids._Sources())
def main():
    app=QApplication.instance() or QApplication([]);rows=[];references={}
    with tempfile.TemporaryDirectory() as temp:
        subprocess.run(["cargo","run","--locked","-p","ingetrazo-tools","--example","followme_acceptance_fixture","--",temp],check=True)
        for name in ["straight","bend","tube","closed-torus"]+[f"sphere-{phi}-{r}-{pr}" for phi in ("0","7_5","33") for r,pr in (("1","1"),("0_5","1"),("1","0_5"))]:
            original=Scene();igz.load_into(original,Path(temp)/(name+"-source.igz"))
            path=[QVector3D(*p) for p in json.loads((Path(temp)/(name+"-path.json")).read_text())]
            assert sweep_profile(original.mesh,original.mesh.faces[0],path,name=="closed-torus" or name.startswith("sphere-"))
            rust=Scene();igz.load_into(rust,Path(temp)/(name+"-result.igz"))
            a,b=manifold(original.mesh),manifold(rust.mesh);difference=(a-b).volume()+(b-a).volume()
            assert difference<1e-5,(name,difference)
            references[name]=a
            rows.append({"case":name,"original_volume":a.volume(),"rust_volume":b.volume(),"symmetric_difference_volume":difference})
        # Straight extrusion cannot satisfy connected bent-path spatial equivalence.
        straight,bend=references["straight"],references["bend"]
        assert (straight-bend).volume()+(bend-straight).volume()>1
    report={"provenance":context(),"passed":True,"negative_controls_detected":1,"cases":rows,"scope":"Straight, right-angle mitre, holed straight tube, circular rectangular torus; independent original native manifold shape oracle. Nine touching/crossing-axis sphere revolutions across three plane orientations and radius pairs. Arbitrary path and interactive workflow remain open."}
    Path("docs/audits/python-followme-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
