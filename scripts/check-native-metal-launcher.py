#!/usr/bin/env python3
"""Actual CLI world submission on Apple CI, with no native opt-in flags.
Only the owned process is terminated. No pixel or clean-close acceptance claim.
"""
import argparse, hashlib, json, os, subprocess, sys, time
from pathlib import Path
parser=argparse.ArgumentParser()
for name in ("binary","document","output"): parser.add_argument(name)
args=parser.parse_args()
assert sys.platform=="darwin","Requires actual macOS"
binary,document=Path(args.binary).resolve(),Path(args.document).resolve()
out=Path(args.output).resolve();out.mkdir(parents=True,exist_ok=False)
env=dict(os.environ,XDG_CONFIG_HOME=str(out/"config"),XDG_DATA_HOME=str(out/"data"),XDG_CACHE_HOME=str(out/"cache"),INGETRAZO_STATE_DIR=str(out/"session"))
for key in ("INGETRAZO_NATIVE_METAL","INGETRAZO_METAL_WORLD","INGETRAZO_METAL_CAD_CALLBACKS"): env.pop(key,None)
document_hash=hashlib.sha256(document.read_bytes()).hexdigest()
marker="Graphics backend: native Metal; first GPU world frame submitted"
refusals=("painting paused:","viewport paused:","compatibility path:")
with (out/"runtime.log").open("w") as log:
 process=subprocess.Popen([str(binary),str(document)],env=env,stdout=log,stderr=log)
 try:
  deadline=time.monotonic()+45
  while time.monotonic()<deadline and process.poll() is None:
   runtime=(out/"runtime.log").read_text()
   if marker in runtime or any(s in runtime for s in refusals): break
   time.sleep(.1)
  assert process.poll() is None,"Launcher exited: "+(out/"runtime.log").read_text()
  runtime=(out/"runtime.log").read_text()
  assert marker in runtime and document.name in runtime,runtime
  assert not any(s in runtime for s in refusals),runtime
 finally:
  if process.poll() is None:
   process.terminate()
   try: process.wait(timeout=10)
   except subprocess.TimeoutExpired: process.kill();process.wait(timeout=5)
assert document_hash==hashlib.sha256(document.read_bytes()).hexdigest()
(out/"result.json").write_text(json.dumps({"passed":True,"world_submitted":True,"opt_in_flags":False,"binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"document":document.name,"document_sha256":document_hash,"termination":"owned process terminated after startup observation","scope":"native packaged CLI world submission, no pixel or clean-close claim"},indent=2)+"\n")
print("PASS: actual packaged CLI selected native Metal and submitted the loaded document")
