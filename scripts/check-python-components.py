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
        assert not unique.groups[1].component
        assert unique.groups[0].mesh is not unique.groups[1].mesh,"Make Unique remained linked"
        assert [g.mesh.vertices[0].position.z() for g in unique.groups]==[3,2,0]
        subprocess.run(["cargo","run","--locked","-p","ingetrazo-gui","--example","component_editor_acceptance_fixture","--",temp],check=True)
        def reopen(name):
            scene=Scene();igz.load_into(scene,directory/(name+".igz"));return scene
        before=reopen("editor-before")
        committed=reopen("editor-committed")
        assert [g.mesh.vertices[0].position.z() for g in committed.groups[:3]]==[5,5,0]
        assert before.groups[3].children[0].uid!=before.groups[4].children[0].uid
        for index in (3,4):
            assert committed.groups[index].children[0].uid==before.groups[index].children[0].uid
            assert committed.groups[index].xform==before.groups[index].xform
            assert committed.groups[index].children[0].xform==before.groups[index].children[0].xform
        assert committed.groups[3].xform[0,3]==4 and committed.groups[4].xform[0,3]==14
        assert committed.groups[3].children[0].xform[0,3]==-2
        assert committed.groups[3].children[0].mesh is committed.groups[0].mesh
        assert committed.groups[4].children[0].mesh is committed.groups[0].mesh
        assert [g.mesh.vertices[0].position.z() for g in reopen("editor-undone").groups[:3]]==[0,0,0]
        parent_unique=reopen("editor-parent-unique")
        assert not parent_unique.groups[4].component
        assert parent_unique.groups[0].mesh.vertices[0].position.z()==6
        assert parent_unique.groups[3].children[0].mesh.vertices[0].position.z()==6
        assert parent_unique.groups[4].children[0].mesh.vertices[0].position.z()==5,"Unique parent still linked to original shared child"
        assert parent_unique.groups[4].children[0].uid==committed.groups[4].children[0].uid
        assert parent_unique.groups[4].xform==committed.groups[4].xform
        assert parent_unique.groups[4].children[0].xform==committed.groups[4].children[0].xform
        unique_editor=reopen("editor-unique")
        assert [g.mesh.vertices[0].position.z() for g in unique_editor.groups[:3]]==[7,5,0]
        assert unique_editor.groups[3].children[0].mesh.vertices[0].position.z()==5
        pasted=reopen("editor-pasted")
        assert pasted.groups[0].mesh is not pasted.groups[5].mesh,"Classic unique group copy must be independent"
        assert pasted.groups[5].mesh.vertices[0].position.z()==7
        assert pasted.groups[1].mesh is pasted.groups[6].mesh,"Linked component copy must preserve sharing"
        assert pasted.groups[1].uid!=pasted.groups[6].uid
        assert pasted.groups[0].uid!=pasted.groups[5].uid
        for suffix in ("linked","unique"):
            path=directory/(suffix+".igz");raw=json.loads(path.read_text())["scene"]
            assert [g["uid"] for g in raw["groups"]]==["persisted-first","persisted-second","persisted-plain"]
            assert [g["opaque_instance"] for g in raw["groups"]]==[67,71,73]
    report={"provenance":context(),"passed":True,"checks":["linked definitions reopen as same Python mesh identity","plain component=false stays independent","shared edit updates both linked instances","Make Unique detaches even identical geometry before later shared edit","instance UID and opaque attributes retained","nested editor commit propagates and Python preserves shared child identity","exact controller undo/redo and cancel preserve document; fragment save blocked","unique classic clipboard copy independent, linked component copy shared, distinct placement IDs","nested definition edit preserves distinct child UIDs and parent/child placement matrices","unique parent subtree resists later original shared-child edits while preserving placements and child identity","unique leaf and parent reopen as ordinary Python groups"],"scope":"Shared triangle and plain group plus nested GUI controller transactions and clipboard, independently reopened by original Python; no rendered UI gesture proof."}
    Path("docs/audits/python-component-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
