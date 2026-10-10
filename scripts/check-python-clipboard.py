#!/usr/bin/env python3
import json,subprocess,tempfile,os,sys
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import igz
def main():
 app=QApplication.instance() or QApplication([])
 with tempfile.TemporaryDirectory() as tmp:
  directory=Path(tmp);source={"igz_format":1,"app_version":"independent","scene":{"edges":[{"a":[10,0,0],"b":[11,0,0],"curve":29,"soft":True,"hidden":True,"vendor_curve":{"preserve":17}}],"faces":[{"vertices":[[0,0,0],[4,0,0],[4,4,0],[0,4,0]],"holes":[[[1,1,0],[1,2,0],[2,2,0],[2,1,0]]],"texture":{"uvw":[1,0,0,3,0,2,0,4],"vendor_projection":73},"vendor_face":{"preserve":23}}]}}
  (directory/"source.igz").write_text(json.dumps(source));subprocess.run(["cargo","run","--locked","-p","ingetrazo-gui","--example","clipboard_acceptance_fixture","--",str(directory)],check=True)
  raw=json.loads((directory/"pasted.igz").read_text());faces=raw["scene"]["faces"];assert len(faces)==2
  translated=next(f for f in faces if min(p[2] for p in f["vertices"])==40)
  assert translated["vendor_face"]=={"preserve":23};assert translated["texture"]["vendor_projection"]==73
  assert translated["texture"]["uvw"]==[1,0,0,-17,0,2,0,-56]
  wires=[e for e in raw["scene"]["edges"] if e.get("vendor_curve")=={"preserve":17}];assert len(wires)==2
  assert len({e["curve"] for e in wires})==2;assert all(e["soft"] and e["hidden"] for e in wires)
  reopened=Scene();igz.load_into(reopened,directory/"pasted.igz");assert len(reopened.mesh.faces)==2
  face=next(f for f in reopened.mesh.faces if all(abs(p.z()-40)<1e-6 for p in f.vertices));assert len(face.holes)==1
  assert sorted((p.x(),p.y(),p.z()) for p in face.vertices)==[(20,30,40),(20,34,40),(24,30,40),(24,34,40)]
  assert sorted((p.x(),p.y(),p.z()) for p in face.holes[0])==[(21,31,40),(21,32,40),(22,31,40),(22,32,40)]
  assert any(e.a.x()==30 and e.b.x()==31 and e.a.y()==30 and e.a.z()==40 for e in reopened.mesh.edges)
 report={"provenance":context(),"passed":True,"scope":"Mixed selected holed loose face and opaque attributed curve: immutable placement preview/cancel, one history step and exact undo/redo; translated affine UV and native original geometry reopen. Same-plane overlap merge, cross-context transforms and system clipboard remain open."}
 Path("docs/audits/python-clipboard-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
