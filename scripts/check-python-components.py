#!/usr/bin/env python3
"""Original reopen of Rust shared-definition updates and Make Unique isolation."""
import argparse,json,os,sys,subprocess,tempfile
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import igz

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("--prebuilt-dir",type=Path,help="Fresh native debug example executables built by the caller")
    args=parser.parse_args()
    if os.environ.get("CARGO_BUILD_TARGET") or os.environ.get("CARGO_PROFILE"):
        parser.error("Component acceptance supports native debug executables only; unset CARGO_BUILD_TARGET/CARGO_PROFILE")
    names=("component_acceptance_fixture","component_editor_acceptance_fixture","component_ci_world_bake_fixture")
    if args.prebuilt_dir is None:
        target=Path(os.environ.get("CARGO_TARGET_DIR","target")).resolve()
        build=subprocess.run(["cargo","build","--locked","--message-format=json-render-diagnostics","-j2","--target-dir",str(target),"-p","ingetrazo-formats","-p","ingetrazo-gui",*[part for name in names for part in ("--example",name)]],check=True,stdout=subprocess.PIPE,text=True)
        binary_dir=target/"debug"/"examples"
        artifacts={name:set() for name in names}
        for line in build.stdout.splitlines():
            message=json.loads(line)
            if message.get("reason")=="compiler-artifact" and message.get("executable") and "example" in message.get("target",{}).get("kind",[]):
                name=message["target"]["name"]
                if name in artifacts:artifacts[name].add(Path(message["executable"]).resolve())
        suffix=".exe" if os.name=="nt" else ""
        for name in names:
            expected=(binary_dir/(name+suffix)).resolve()
            if artifacts[name]!={expected}:
                parser.error(f"Unsupported target configuration or missing Cargo artifact for {name}: {sorted(map(str,artifacts[name]))}; expected {expected}")
    else:
        binary_dir=args.prebuilt_dir.resolve()
    suffix=".exe" if os.name=="nt" else ""
    binaries={name:binary_dir/(name+suffix) for name in names}
    for binary in binaries.values():
        if not binary.is_file() or not os.access(binary,os.X_OK):
            parser.error(f"Required prebuilt executable missing or not executable: {binary}")
    app=QApplication.instance() or QApplication([])
    subprocess.run([sys.executable,"scripts/probe-python-component-ci-bake.py"],check=True)
    subprocess.run([sys.executable,"scripts/check-component-ci-world-bake.py","--prebuilt-dir",str(binary_dir)],check=True)
    with tempfile.TemporaryDirectory() as temp:
        directory=Path(temp);identity=[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]
        source={"igz_format":1,"app_version":"independent-original","scene":{"opaque_document":{"retain":[83,89]},"faces":[],"edges":[{"a":[-3,0,0],"b":[-2,0,0]}],"protos":[{"edges":[],"faces":[{"vertices":[[0,0,0],[1,0,0],[0,1,0]],"opaque_face":61}]}],"groups":[{"name":name,"uid":"persisted-"+name,"component":component,"proto":0,"xform":identity,"opaque_instance":value} for name,component,value in [("first",True,67),("second",True,71),("plain",False,73)]]}}
        (directory/"component-source.igz").write_text(json.dumps(source))
        subprocess.run([str(binaries["component_acceptance_fixture"]),temp],check=True)
        linked=Scene();igz.load_into(linked,directory/"linked.igz")
        assert linked.groups[0].mesh is linked.groups[1].mesh,"Linked instances lost actual shared native mesh"
        assert linked.groups[0].mesh is not linked.groups[2].mesh and not linked.groups[2].component
        assert linked.groups[0].mesh.vertices[0].position.z()==2 and linked.groups[2].mesh.vertices[0].position.z()==0
        unique=Scene();igz.load_into(unique,directory/"unique.igz")
        assert not unique.groups[1].component
        assert unique.groups[0].mesh is not unique.groups[1].mesh,"Make Unique remained linked"
        assert [g.mesh.vertices[0].position.z() for g in unique.groups]==[3,2,0]
        subprocess.run([str(binaries["component_editor_acceptance_fixture"]),temp],check=True)
        def reopen(name):
            scene=Scene();igz.load_into(scene,directory/(name+".igz"));return scene
        before=reopen("editor-before")
        committed=reopen("editor-committed")
        assert [g.mesh.vertices[0].position.z() for g in committed.groups[:3]]==[5,5,5]
        assert before.groups[3].children[0].uid!=before.groups[4].children[0].uid
        for index in (3,4):
            assert committed.groups[index].children[0].uid==before.groups[index].children[0].uid
            expected_parent=before.groups[index].xform if index==4 else type(before.groups[index].xform)()
            expected_child=before.groups[index].children[0].xform if index==4 else before.groups[index].xform*before.groups[index].children[0].xform
            assert committed.groups[index].xform==expected_parent
            assert committed.groups[index].children[0].xform==expected_child
        assert committed.groups[3].xform[0,3]==0 and committed.groups[4].xform[0,3]==14
        assert committed.groups[3].children[0].xform[0,3]==2
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
        context_saved=reopen("editor-context-save")
        assert len(context_saved.groups)==5,"Context Save wrote a fragment"
        assert [g.uid for g in context_saved.groups]==[g.uid for g in committed.groups]
        assert [g.mesh.vertices[0].position.z() for g in context_saved.groups[:3]]==[9,9,9]
        for index in (3,4):
            assert len(context_saved.groups[index].children)==1
            assert context_saved.groups[index].children[0].uid==committed.groups[index].children[0].uid
            assert context_saved.groups[index].children[0].mesh is context_saved.groups[0].mesh
            assert context_saved.groups[index].children[0].mesh.vertices[0].position.z()==9
        assert [[list(e.v0.position.toTuple()),list(e.v1.position.toTuple())] for e in context_saved.mesh.edges]==[[[-3,0,0],[-2,0,0]]]
        raw_context=json.loads((directory/"editor-context-save.igz").read_text())["scene"]
        assert raw_context["opaque_document"]=={"retain":[83,89]}
        assert [g["opaque_instance"] for g in raw_context["groups"][:3]]==[67,71,73]
        assert all(raw_context["protos"][g["proto"]]["faces"][0]["opaque_face"]==61 for g in raw_context["groups"][:3])
        unique_editor=reopen("editor-unique")
        assert [g.mesh.vertices[0].position.z() for g in unique_editor.groups[:3]]==[7,5,5]
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
    report={"provenance":context(),"passed":True,"checks":["linked definitions reopen as same Python mesh identity","explicit legacy definition replacement keeps plain placement independent in that fixture","shared edit updates both linked instances","Make Unique detaches even identical geometry before later shared edit","instance UID and opaque attributes retained","nested editor commit propagates and Python preserves shared child identity","source-owned mesh/placement undo/redo preserves full remaining document, cleared selection and advancing revision; cancel exact and complete active-context Save exits all contexts, preserves root geometry/identities/opaque data and reopens shared shapes","unique classic clipboard copy independent, linked component copy shared, distinct placement IDs","nested world edit preserves distinct UIDs, bakes selected container placement into children, preserves sibling placements","unique parent subtree resists later original shared-child edits while preserving placements and child identity","unique leaf and parent reopen as ordinary Python groups"],"scope":"Shared triangle and plain group plus nested GUI controller transactions and clipboard, independently reopened by original Python; fresh actual original translated/rotated/tilted-scaled WORLD controls and six rejected affine, precision and history-presentation mutations; no rendered UI gesture proof."}
    Path("docs/audits/python-component-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
