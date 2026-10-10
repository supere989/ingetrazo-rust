#!/usr/bin/env python3
"""Actual ash host production composer; controlled public actions, not menu gestures."""
import os,sys,subprocess,time,json,hashlib
from pathlib import Path
from PySide6.QtGui import QImage
binary=Path(sys.argv[1]).resolve();worker=Path(sys.argv[2]).resolve();out=Path(sys.argv[3]).resolve();out.mkdir(parents=True,exist_ok=True)
x=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1400x1000x24"],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
display=":"+x.stdout.readline().strip()
env=dict(os.environ,DISPLAY=display,XDG_CONFIG_HOME=str(out/"config"),INGETRAZO_ACCEPTANCE_OUT=str(out),INGETRAZO_HLR_WORKER_EXE=str(worker))
states={};images={}
with (out/"native.log").open("w") as log:
 p=subprocess.Popen([str(binary)],env=env,stdout=log,stderr=log)
 try:
  for phase in ["baseline","edited","undo","context-cancel","recovered"]:
   (out/"phase").write_text(phase);deadline=time.monotonic()+30
   while True:
    assert p.poll() is None,"native application exited"
    try:state=json.loads((out/"state.json").read_text())
    except (FileNotFoundError,json.JSONDecodeError):state={}
    if state.get("phase")==phase:break
    assert time.monotonic()<deadline,f"phase timeout {phase}";time.sleep(.05)
   time.sleep(.4)
   subprocess.run(["import","-display",display,"-window","root",str(out/(phase+".png"))],check=True)
   states[phase]=state
   image=QImage(str(out/(phase+".png"))).copy(220,210,590,410).convertToFormat(QImage.Format.Format_RGBA8888)
   images[phase]=bytes(image.constBits())
  assert images["edited"]!=images["baseline"],"edit produced stale native pixels"
  assert images["undo"]==images["baseline"],"Undo failed to restore native paper"
  assert images["context-cancel"]==images["baseline"],"component cancellation leaked local ink"
  assert images["recovered"]!=images["baseline"],"recovery produced stale native pixels"
  ids=states["baseline"]["pids"];assert len(ids)==2
  assert all(s["pids"]==ids for s in states.values())
  jobs=states["recovered"]["jobs"];time.sleep(.5);assert json.loads((out/"state.json").read_text())["jobs"]==jobs
  result={"states":states,"native_binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"worker_sha256":hashlib.sha256(worker.read_bytes()).hexdigest(),"source":subprocess.check_output(["git","rev-parse","HEAD"],text=True).strip(),"scope":"ash software Vulkan production update_ui + public edit/Undo/component cancel/recovery actions; pixel ROI and serial ink checks; no menu gesture claim"}
  (out/"result.json").write_text(json.dumps(result,indent=2)+"\n")
  print("PASS actual Vulkan composer: edit/Undo/component cancel/recovery pixels, exact serial ink, persistent workers and warm cache")
 finally:
  if p.poll() is None:p.terminate();p.wait(timeout=5)
  x.terminate();x.wait(timeout=5)
