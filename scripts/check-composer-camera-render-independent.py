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
  for phase in ["baseline","navigated","restored","missing","bound","bound-live-change","order-behind","order-cover","order-tie","order-hidden","border-on"]:
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
   raw=QImage(str(out/(phase+".png")));assert not raw.isNull() and raw.width()>=810 and raw.height()>=620
   image=raw.copy(220,210,590,410).convertToFormat(QImage.Format.Format_RGBA8888)
   images[phase]=bytes(image.constBits())
  assert images["navigated"]!=images["baseline"],"live camera navigation stale"
  assert images["restored"]==images["baseline"],"live camera restore differs"
  assert images["missing"]==images["baseline"],"missing saved view did not use live camera"
  assert images["bound"]==images["navigated"],"saved camera not applied"
  assert images["bound-live-change"]==images["bound"],"saved frame changed with live camera"
  assert images["order-behind"]==images["baseline"],"lower z did not paint behind"
  assert images["order-cover"]==images["navigated"],"higher z did not occlude"
  assert images["order-tie"]==images["order-cover"],"equal frame z did not retain category order"
  assert images["order-hidden"]==images["baseline"],"hidden frame painted"
  assert images["border-on"]!=images["baseline"],"border flag did not change rendered pixels"
  ids=states["baseline"]["pids"];assert len(ids)==2
  assert all(s["pids"]==ids for s in states.values())
  jobs=states["border-on"]["jobs"];time.sleep(.5);assert json.loads((out/"state.json").read_text())["jobs"]==jobs
  result={"passed":True,"states":states,"native_binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"worker_sha256":hashlib.sha256(worker.read_bytes()).hexdigest(),"source":subprocess.check_output(["git","rev-parse","HEAD"],text=True).strip(),"scope":"ash software Vulkan production update_ui, controlled live/saved camera and same-category z/hidden actions; exact pixel ROI comparisons, persistent workers; no original pixel/menu gesture/all-painter claim"}
  (out/"result.json").write_text(json.dumps(result,indent=2)+"\n")
  print("PASS actual Vulkan composer: live navigation/restoration/missing/bound camera pixels, controlled camera/order pixel equality, persistent workers and warm cache")
 finally:
  if p.poll() is None:
   p.terminate()
   try:p.wait(timeout=5)
   except subprocess.TimeoutExpired:p.kill();p.wait(timeout=3)
  x.terminate();x.wait(timeout=5)
