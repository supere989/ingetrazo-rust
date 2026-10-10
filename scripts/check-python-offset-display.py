#!/usr/bin/env python3
"""Original edge classification of actual committed native Offset geometry."""
import os,sys,json
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from core.scene import Scene
from core.hlr import collect_geometry
from formats import igz
def segment(a,b):return tuple(sorted(tuple(round(x,5) for x in p) for p in (a,b)))
def main():
 source=Path("docs/audits/offset-render/committed.igz");assert source.is_file(),"native rendered fixture must run first"
 s=Scene();igz.load_into(s,source);tris,hard,soft=collect_geometry(s)
 expected=json.loads(Path("docs/audits/offset-render-semantic-evidence.json").read_text())["preview"]["preview"]
 classified={segment(a,b) for a,b in hard}
 assert all(segment(a,b) in classified for a,b in expected),"original treats hard coplanar inset as hard display geometry"
 assert len(hard)==8 and not soft,(len(hard),len(soft))
 report={"provenance":context(),"passed":True,"original_hard_edges":len(hard),"original_soft_edges":len(soft),"scope":"Original core.hlr.collect_geometry classifies all four coplanar Offset inset edges as hard, validating committed boundary visibility independently of soft-edge toggle."}
 Path("docs/audits/python-offset-display-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
