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
        for name in ["straight","bend","tube","closed-torus"]+[f"sphere-{phi}-{r}-{pr}" for phi in ("0","7_5","33") for r,pr in (("1","1"),("0_5","1"),("1","0_5"))]+["sphere-wire0-1-1","sphere-wire7_5-1-1","attached-straight","attached-bend"]:
            original=Scene();igz.load_into(original,Path(temp)/(name+"-source.igz"))
            path=[QVector3D(*p) for p in json.loads((Path(temp)/(name+"-path.json")).read_text())]
            face=next(f for f in original.mesh.faces if all(abs(v.z()-1)<1e-8 for v in f.vertices)) if name.startswith("attached-") else original.mesh.faces[0]
            assert sweep_profile(original.mesh,face,path,name=="closed-torus" or name.startswith("sphere-"))
            rust=Scene();igz.load_into(rust,Path(temp)/(name+"-result.igz"))
            if "wire" in name:
                assert not [e for e in original.mesh.edges if not e.soft or not e.faces]
                assert not [e for e in rust.mesh.edges if not e.soft or not e.faces],(name,"hard or loose sphere seams")
            a,b=manifold(original.mesh),manifold(rust.mesh);difference=(a-b).volume()+(b-a).volume()
            assert difference<1e-5,(name,difference)
            references[name]=a
            rows.append({"case":name,"original_volume":a.volume(),"rust_volume":b.volume(),"symmetric_difference_volume":difference})

        def ring(vertices):
            p=[tuple(round(v,5) for v in (x.x(),x.y(),x.z())) for x in vertices]
            return min(tuple(p[i:]+p[:i]) for i in range(len(p)))
        def face_set(mesh):return sorted((ring(f.vertices),tuple(sorted(ring(h) for h in f.holes))) for f in mesh.faces)
        for name in ["open-perpendicular","open-perpendicular-bend","open-coplanar"]:
            original=Scene();igz.load_into(original,Path(temp)/(name+"-source.igz"))
            path=[QVector3D(*p) for p in json.loads((Path(temp)/(name+"-path.json")).read_text())]
            assert sweep_profile(original.mesh,original.mesh.faces[0],path,False)
            rust=Scene();igz.load_into(rust,Path(temp)/(name+"-result.igz"))
            assert face_set(original.mesh)==face_set(rust.mesh),(name,"ordered complete surface geometry")
            assert len(rust.mesh.faces)>2,"unchanged sheets negative control"
            rows.append({"case":name,"original_faces":len(original.mesh.faces),"rust_faces":len(rust.mesh.faces),"ordered_surface_equality":True})
        # Straight extrusion cannot satisfy connected bent-path spatial equivalence.
        straight,bend=references["straight"],references["bend"]
        assert (straight-bend).volume()+(bend-straight).volume()>1
    report={"provenance":context(),"passed":True,"negative_controls_detected":1,"cases":rows,"scope":"Straight, right-angle mitre, holed straight tube, circular rectangular torus; independent original native manifold shape oracle. Nine touching/crossing-axis sphere revolutions across three plane orientations and radius pairs. Attached closed cube cap straight and bent sweeps also match original native spatial solids. Three open perpendicular/coplanar sheet attachments match complete original ordered face geometry. Arbitrary attachments remain open."}
    Path("docs/audits/python-followme-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
