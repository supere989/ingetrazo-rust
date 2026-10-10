#!/usr/bin/env python3
"""Bounded software controls for the production headless Vulkan probe; not hardware proof."""
import argparse, hashlib, json, subprocess, time
from pathlib import Path
from png_capture import valid_png
p=argparse.ArgumentParser(); p.add_argument('binary'); p.add_argument('output'); a=p.parse_args()
binary=Path(a.binary).resolve(); out=Path(a.output).resolve(); out.mkdir(parents=True,exist_ok=True)
(out/'depth.png').unlink(missing_ok=True)
rows=[]
for name,extra,expected in [('positive',['--software-fixture','--output',str(out/'depth.png')],0),('wrong-vendor',['--software-fixture','--vendor','0x10de'],1),('unsignaled-timeout',['--software-fixture','--timeout-control'],124)]:
 command=[str(binary),*extra]; started=time.monotonic()
 try:
  r=subprocess.run(command,capture_output=True,text=True,timeout=20)
 except subprocess.TimeoutExpired as error:
  (out/(name+'.log')).write_bytes((error.stdout or b'')+(error.stderr or b''))
  (out/'failure.json').write_text(json.dumps(dict(case=name,command=command,error='external timeout',elapsed=time.monotonic()-started),indent=2)+'\n')
  raise
 elapsed=time.monotonic()-started; log=r.stdout+r.stderr; (out/(name+'.log')).write_text(log)
 assert r.returncode==expected,(name,r.returncode,log)
 if name=='positive':
  assert 'llvmpipe' in log and 'PASS17MiB actualGPUcopy/readback' in log and 'PASS native world draw2' in log
  assert valid_png(out/'depth.png'),'Incomplete depth image'
 elif name=='wrong-vendor': assert 'software fallback refused' in log
 else: assert 'GPU diagnostic fence exceeded' in log and 4.5<=elapsed<15,(name,elapsed)
 rows.append(dict(case=name,exit_code=r.returncode,elapsed_seconds=elapsed,command=command))
(out/'result.json').write_text(json.dumps(dict(binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),cases=rows,scope='Software production resource/depth and refusal/timeout controls only; hardware and Metal remain separate'),indent=2)+'\n')
print('PASS software headless resource/depth, adapter refusal and bounded timeout controls')
