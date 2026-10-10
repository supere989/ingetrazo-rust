#!/usr/bin/env python3
"""Original QPainter versus actual production eframe vector widths and dashes."""
import sys,os,json,tempfile,subprocess,hashlib,shutil
from pathlib import Path
from PySide6.QtGui import QImage
source=Path(sys.argv[1]).resolve();root=Path("docs/audits/composer-ink-render");root.mkdir(exist_ok=True)
subprocess.run(["cargo","build","--locked","-p","ingetrazo-gui","--example","composer_ink_acceptance_fixture"],cwd=source,check=True)
def runs(values):
 out=[]
 for v in values:
  if out and v==out[-1][1]+1:out[-1][1]=v
  else:out.append([v,v])
 return out
def measure(path):
 i=QImage(str(path));assert (i.width(),i.height())==(800,800)
 dark=lambda x,y:i.pixelColor(x,y).red()<128
 points=[(x,y)for x in range(800)for y in range(800)if dark(x,y)]
 return {"pixels":len(points),"bounds":[min(x for x,y in points),max(x for x,y in points),min(y for x,y in points),max(y for x,y in points)],"solid_width":sum(dark(300,y)for y in range(265,295)),"dash_runs":runs([x for x in range(378,431)if dark(x,440)])}
def matches(a,b):
 return abs(a["pixels"]-b["pixels"])/a["pixels"]<.05 and all(abs(x-y)<=1 for x,y in zip(a["bounds"],b["bounds"])) and abs(a["solid_width"]-b["solid_width"])<=1 and len(a["dash_runs"])==len(b["dash_runs"]) and all(abs(x-y)<=1 for r,s in zip(a["dash_runs"],b["dash_runs"])for x,y in zip(r,s))
with tempfile.TemporaryDirectory()as d:
 d=Path(d);subprocess.run([sys.executable,str(source/"scripts/reference-composer-hlr-fixture.py"),"--original",str(source/"ingetrazo"),"--output",str(d)],check=True)
 binary=d/"native";shutil.copy2(source/"target/debug/examples/composer_ink_acceptance_fixture",binary);rows=[]
 for width in [.18,.5,1.]:
  original=root/(str(width)+".original.png");native=root/(str(width)+".native.png")
  subprocess.run([sys.executable,"scripts/reference-composer-ink-fixture.py",str(d/"wall_hidden.reference.json"),str(original),str(width)],env=dict(os.environ,INGETRAZO_ORIGINAL_ROOT=str(source/"ingetrazo")),check=True)
  env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE="1",XDG_RUNTIME_DIR=str(d/"runtime"),XDG_CONFIG_HOME=str(d/"config"),XDG_CACHE_HOME=str(d/"cache"));Path(env["XDG_RUNTIME_DIR"]).mkdir(exist_ok=True,mode=0o700)
  subprocess.run(["xvfb-run","-a",str(binary),str(d/"wall_hidden.igz"),str(native.resolve()),str(width)],env=env,check=True)
  a,b=measure(original),measure(native)
  same=matches(a,b)
  rows.append({"width_mm":width,"original":a,"native":b,"match":same})
 controls=[]
 for name,field,value in [("wrong-paper-width","solid_width",20),("continuous-hidden-line","dash_runs",[[378,430]]),("missing-ink","pixels",0)]:
  changed=dict(rows[1]["native"]);changed[field]=value;assert not matches(rows[1]["original"],changed);controls.append({"case":name,"detected":True})
 r={"negative_controls":controls,"native_revision":subprocess.check_output(["git","rev-parse","HEAD"],cwd=source,text=True).strip(),"binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"passed":all(r["match"]for r in rows),"cases":rows,"scope":"Actual original paint_frame_mm versus production composer::render_model_projection in native eframe. Eight pixels/mm, three paper widths, front opaque wall and hidden box. Total ink tolerance5%, solid width/bounds/dash endpoints1pixel for raster AA. Cut-class pens, hatch/fill, other DPI, selected frames, raster styles and exports remain separate."}
 Path("docs/audits/composer-ink-evidence.json").write_text(json.dumps(r,indent=2)+"\n");print(json.dumps(r,indent=2));assert r["passed"]
