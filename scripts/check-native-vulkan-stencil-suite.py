#!/usr/bin/env python3
"""Fresh native production stencil cases, paired with the pinned original captures."""
import argparse,os,subprocess,sys,signal,hashlib,json
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('binary');p.add_argument('output');p.add_argument('--original',default='docs/audits/section-original-gl/original-gl-results.json');a=p.parse_args()
out=Path(a.output);out.mkdir(parents=True,exist_ok=True)
cases=['closed','shared-slab','shared-slab-reversed','transparent','duplicate','coplanar-backface','closed-iso','shared-slab-iso','shared-slab-reversed-iso','closed-front']
for case in cases:
 env=dict(os.environ,INGETRAZO_SECTION_CASE=case)
 process=subprocess.Popen([sys.executable,'scripts/check-native-vulkan-startup.py',a.binary,str(out/('msaa-'+case))],env=env,start_new_session=True)
 try:
  code=process.wait(timeout=20)
 except subprocess.TimeoutExpired:
  os.killpg(process.pid,signal.SIGKILL);process.wait();raise
 if code:raise subprocess.CalledProcessError(code,process.args)
result=subprocess.check_output([sys.executable,'scripts/check-native-vulkan-stencil.py',a.original,str(out),'docs/audits/stencil-independent/wrong-winding-historical.png'],text=True)
(out/'comparison.json').write_text(result)
print(result)
logs={case:[line for line in (out/('msaa-'+case)/'startup.log').read_text().splitlines() if 'Native Vulkan adapter:' in line or 'Native world MSAA:' in line] for case in cases}
(out/'provenance.json').write_text(json.dumps({'source':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'binary_sha256':hashlib.sha256(Path(a.binary).read_bytes()).hexdigest(),'driver_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'adapters_and_msaa':logs,'limits':'software Vulkan ten bounded fixtures; exact zero and 5% positive; native post-fill contours/hardware/Metal remain open'},indent=2)+'\n')
