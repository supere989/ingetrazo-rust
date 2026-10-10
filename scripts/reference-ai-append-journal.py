#!/usr/bin/env python3
import os,sys,json,types
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
s=Scene();vp=types.SimpleNamespace(scene=s,history=History(s),notify_scene_changed=lambda:None);scope={};results=[]
cases=json.loads((Path(__file__).resolve().parents[1]/"tools/ai-interpreter-proof/transaction-cases.json").read_text())
for case in cases:
 r=run_transactional(vp,case["code"],scope)
 vertices=list(s.mesh.vertices);index={id(v):i for i,v in enumerate(vertices)}
 results.append({"name":case["name"],"changed":r["changed"],"failed":r["error"] is not None,"stdout":r["stdout"],"vertices":[[v.position.x(),v.position.y(),v.position.z()]for v in vertices],"edges":[[index[id(e.v0)],index[id(e.v1)]]for e in s.mesh.edges],"traceback":r["stderr"]})
print(json.dumps({"cases":results},indent=2))
