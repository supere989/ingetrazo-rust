#!/usr/bin/env python3
"""Fresh original HLR/IGZ and actual native application selected-frame DXF."""
import sys,os,json,subprocess,hashlib
from pathlib import Path
import ezdxf
from acceptance_context import context
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from formats.dxf_out import save_dxf_layers
root=Path("docs/audits/sheet-view-production")
root.mkdir(parents=True,exist_ok=True)
subprocess.run([sys.executable,"scripts/reference-composer-hlr-fixture.py","--original",str(Path("ingetrazo").resolve()),"--output",str(root)],check=True)
subprocess.run(["cargo","build","--locked","-p","ingetrazo-gui","--example","composer_dxf_export_fixture","-p","ingetrazo-render","--bin","ingetrazo-render-worker"],check=True)
target=Path(os.environ.get("CARGO_TARGET_DIR","target"))/"debug"
exe=target/"examples/composer_dxf_export_fixture"
subprocess.run([str(exe.resolve()),str(root),str((target/"ingetrazo-render-worker").resolve())],check=True)
rows=[]
for path in sorted(root.glob("*.reference.json")):
 r=json.loads(path.read_text());name=r["name"]
 for hidden in (False,True):
  d=r["drawings"][str(hidden).lower()];groups=[]
  for kind,suffix in ((0,""),(1,"-PERFIL"),(2,"-CORTE"),(3,"-OCULTAS")):
   if kind==3 and not hidden:continue
   groups.append(("Export"+suffix,[s for s,k in zip(d["segs"],d["kinds"]) if k==kind],"DASHED" if kind==3 else "CONTINUOUS"))
  original=root/f"{name}.{str(hidden).lower()}.original.dxf";save_dxf_layers(original,groups)
  a=ezdxf.readfile(original);b=ezdxf.readfile(root/f"{name}.{str(hidden).lower()}.native.dxf")
  def layers(doc):return [(x.dxf.name,x.dxf.linetype) for x in doc.layers if x.dxf.name not in ("0","Defpoints")]
  same=layers(a)==layers(b)
  remaining=[(x.dxf.layer,list(x.dxf.start)[:2],list(x.dxf.end)[:2]) for x in b.modelspace()]
  same= same and len(a.modelspace())==len(remaining)
  def close(x,y):return max(abs(t-s) for t,s in zip(x,y))<=2e-5
  for line in a.modelspace():
   u0=list(line.dxf.start)[:2];v0=list(line.dxf.end)[:2]
   for i,(layer,u,v) in enumerate(remaining):
    if layer==line.dxf.layer and ((close(u0,u) and close(v0,v)) or (close(u0,v) and close(v0,u))):remaining.pop(i);break
   else:same=False;break
  rows.append({"name":name,"hidden":hidden,"passed":same and not remaining,"original_lines":len(a.modelspace()),"native_lines":len(b.modelspace())})
assert len(rows)==26
report={"passed":all(x["passed"] for x in rows),"provenance":context(),"fixture_sha256":hashlib.sha256(exe.read_bytes()).hexdigest(),"reader":ezdxf.__version__,"tolerance_metres":2e-5,"cases":rows,"scope":"Fresh original HLR/source writer and actual native saved-frame export. Camera-plane metres/class endpoints, stable cached worker PIDs and selection/I/O failures. Actual UI gesture and PDF/image remain open."}
(root/"reader-results.json").write_text(json.dumps(report,indent=2)+"\n")
assert report["passed"],rows
print("Actual composer selected-view DXF 26 independent reader cases PASS")
