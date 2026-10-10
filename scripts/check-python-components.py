#!/usr/bin/env python3
"""Original reopen of Rust shared-definition updates and Make Unique isolation."""
import json,os,sys,subprocess,tempfile
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import igz

def main():
    app=QApplication.instance() or QApplication([])
    with tempfile.TemporaryDirectory() as temp:
        directory=Path(temp);identity=[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]
        source={"igz_format":1,"app_version":"independent-original","scene":{"faces":[],"edges":[],"protos":[{"edges":[],"faces":[{"vertices":[[0,0,0],[1,0,0],[0,1,0]],"opaque_face":61}]}],"groups":[{"name":name,"uid":"persisted-"+name,"component":component,"proto":0,"xform":identity,"opaque_instance":value} for name,component,value in [("first",True,67),("second",True,71),("plain",False,73)]]}}
        (directory/"component-source.igz").write_text(json.dumps(source))
        subprocess.run(["cargo","run","--locked","-p","ingetrazo-formats","--example","component_acceptance_fixture","--",temp],check=True)
        linked=Scene();igz.load_into(linked,directory/"linked.igz")
        assert linked.groups[0].mesh is linked.groups[1].mesh,"Linked instances lost actual shared native mesh"
        assert linked.groups[0].mesh is not linked.groups[2].mesh and not linked.groups[2].component
        assert linked.groups[0].mesh.vertices[0].position.z()==2 and linked.groups[2].mesh.vertices[0].position.z()==0
        unique=Scene();igz.load_into(unique,directory/"unique.igz")
        assert unique.groups[0].mesh is not unique.groups[1].mesh,"Make Unique remained linked"
        assert [g.mesh.vertices[0].position.z() for g in unique.groups]==[3,2,0]
        for suffix in ("linked","unique"):
            path=directory/(suffix+".igz");raw=json.loads(path.read_text())["scene"]
            assert [g["uid"] for g in raw["groups"]]==["persisted-first","persisted-second","persisted-plain"]
            assert [g["opaque_instance"] for g in raw["groups"]]==[67,71,73]
    report={"provenance":context(),"passed":True,"checks":["linked definitions reopen as same Python mesh identity","plain component=false stays independent","shared edit updates both linked instances","Make Unique detaches even identical geometry before later shared edit","instance UID and opaque attributes retained"],"scope":"One shared triangle definition and plain group; no nested instance/editor/clipboard/undo proof."}
    Path("docs/audits/python-component-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
