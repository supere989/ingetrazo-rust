#!/usr/bin/env python3
import subprocess,json,os,time,hashlib
from pathlib import Path
root=Path(__file__).resolve().parents[4];work=Path('/tmp/ingetrazo-shared-cache-camera-final');work.mkdir(exist_ok=False)
sha=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
env=dict(os.environ,CARGO_TARGET_DIR='/tmp/ingetrazo-native-font-merge-target',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0')
stages=[]
for name,cmd in [('native-vulkan-tests',['cargo','test','--locked','-p','ingetrazo-gui','--lib','native_vulkan','-j2']),('named-app-build',['cargo','build','--locked','-p','ingetrazo-gui','--example','native_vulkan_face_sweep_application_probe','-j2'])]:
 start=time.monotonic()
 with(work/(name+'.log')).open('w')as f:r=subprocess.run(cmd,cwd=root,env=env,stdout=f,stderr=subprocess.STDOUT)
 stages.append({'name':name,'source':sha,'command':cmd,'returncode':r.returncode,'seconds':time.monotonic()-start});(work/'stages.json').write_text(json.dumps(stages,indent=2)+'\n');print(name,r.returncode,flush=True)
 if r.returncode:raise SystemExit(r.returncode)
binary=Path(env['CARGO_TARGET_DIR'])/'debug/examples/native_vulkan_face_sweep_application_probe'
(work/'build-provenance.json').write_text(json.dumps({'source':sha,'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'scope':'Native Vulkan consumer changed after full919test checkpoint; focused tests/build only, no second broad suite'},indent=2)+'\n')
