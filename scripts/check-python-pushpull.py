#!/usr/bin/env python3
"""Original interactive classify/commit geometry versus Rust PushPull outputs."""
import json,os,sys,subprocess,tempfile
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from core.group import Group
from core import solids
from formats import igz
from tests.test_pushpull_subtractive import _push_real

def manifold(mesh):return solids._to_manifold(Group(mesh),solids._Sources())
def main():
    app=QApplication.instance() or QApplication([]);rows=[]
    with tempfile.TemporaryDirectory() as temp:
        subprocess.run(["cargo","run","--locked","-p","ingetrazo-tools","--example","pushpull_acceptance_fixture","--",temp],check=True)
        for name,distance,face in [("cap-out",.5,1),("cap-in",-.5,1),("pocket",-.5,6),("through",-1,6)]:
            original=Scene();igz.load_into(original,Path(temp)/(name+"-source.igz"));_push_real(original,original.mesh.faces[face],distance)
            rust=Scene();igz.load_into(rust,Path(temp)/(name+"-result.igz"))
            a,b=manifold(original.mesh),manifold(rust.mesh);difference=(a-b).volume()+(b-a).volume()
            assert difference<1e-6,(name,difference)
            rows.append({"case":name,"original_volume":a.volume(),"rust_volume":b.volume(),"symmetric_difference_volume":difference})
    report={"provenance":context(),"passed":True,"cases":rows,"scope":"Closed cube cap extension/recess, blind square pocket and through-cut; original tool classify+commit and independent native manifold shape oracle. No arbitrary/GUI/undo parity claim."}
    Path("docs/audits/python-pushpull-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
