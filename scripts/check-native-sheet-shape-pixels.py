#!/usr/bin/env python3
"""Strict actual native/source local shape pixels; failures remain failures."""
import os,sys,subprocess,time,json,hashlib
from pathlib import Path
from PySide6.QtGui import QImage
EXPECTED=["rect","rounded","ellipse","polygon-clamped","line-inverted","arrow","terrain-ticks","terrain-hatch-inverted","terrain-band"]
def validate_cases(rows):
 assert [r["name"] for r in rows]==EXPECTED,"Missing, duplicated, extra or reordered mandatory shape case"

binary=Path(sys.argv[1]).resolve();source=Path(sys.argv[2]).resolve();out=Path(sys.argv[3]).resolve();out.mkdir(parents=True,exist_ok=True)
rows=json.loads((source/"original-commands.json").read_text());validate_cases(rows);x=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1400x1000x24"],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True);display=":"+x.stdout.readline().strip();env=dict(os.environ,DISPLAY=display,INGETRAZO_SHAPE_RECIPES=str(source/"recipes.json"),INGETRAZO_SHAPE_OUT=str(out));cases=[]
with (out/"native.log").open("w")as log:
 p=subprocess.Popen([str(binary)],env=env,stdout=log,stderr=log)
 try:
  for phase,row in enumerate(rows):
   (out/"phase.next").write_text(str(phase));os.replace(out/"phase.next",out/"phase");deadline=time.monotonic()+30
   while True:
    assert p.poll()is None
    try:state=json.loads((out/"state.json").read_text())
    except (FileNotFoundError,json.JSONDecodeError):state={}
    if state.get("phase")==phase:break
    assert time.monotonic()<deadline;time.sleep(.05)
   time.sleep(.25);capture=out/(row["name"]+".png");subprocess.run(["import","-display",display,"-window","root",str(capture)],check=True)
   metrics=json.loads((out/"state.json").read_text());assert metrics.get("phase")==phase and metrics.get("direct"),"Actual direct CAD path not observed"
   startup_density_upload=int(float(os.environ.get('INGETRAZO_SHAPE_PPP','1'))!=1);assert metrics['uploads']==phase+1+startup_density_upload,"Unexpected repeated/cold geometry uploads"
   before=metrics['uploads'];time.sleep(.15);warm=json.loads((out/"state.json").read_text());assert warm['uploads']==before and warm['cache_hits']>metrics['cache_hits'],"Warm geometry reused incorrectly"
   a=QImage(str(source/(row["name"]+".png"))).convertToFormat(QImage.Format.Format_RGBA8888);b=QImage(str(capture)).copy(0,0,600,500).convertToFormat(QImage.Format.Format_RGBA8888);assert not a.isNull()and not b.isNull()and a.size()==b.size()
   aa,bb=bytes(a.constBits()),bytes(b.constBits());changed=sum(aa[i:i+4]!=bb[i:i+4]for i in range(0,len(aa),4));cases.append({"name":row["name"],"different_pixels":changed,"passed":changed==0,"gpu_uploads":warm["uploads"],"gpu_cached_bytes":warm["cached_bytes"],"warm_zero_uploads":True})
  validate_cases(cases)
  rejected=0
  for bad in [cases[:-1],cases[:-1]+[cases[0]],list(reversed(cases))]:
   try:validate_cases(bad)
   except AssertionError:rejected+=1
   else:raise AssertionError("Shape inventory negative accepted")
  report={"mandatory_cases":len(EXPECTED),"inventory_negative_controls":rejected,"passed":all(c["passed"]for c in cases),"cases":cases,"binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"boundary":"Actual ash software Vulkan local physical shape renderer vs actual original QPainter PNG; strict pixels, no tolerance or full-sheet/export claim"};(out/"result.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report));assert report["passed"],"Shape raster pixel parity remains open"
 finally:
  if p.poll()is None:p.terminate();p.wait(timeout=5)
  x.terminate();x.wait(timeout=5)
