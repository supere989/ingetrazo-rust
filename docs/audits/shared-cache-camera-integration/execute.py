#!/usr/bin/env python3
"""One stable integrated suite and software Vulkan app controls; hardware replay separate."""
import hashlib,json,os,subprocess,time,zipfile
from pathlib import Path
root=Path(__file__).resolve().parents[3]
work=Path('/tmp/ingetrazo-shared-cache-camera-integration')
work.mkdir(exist_ok=False)
sha=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
pin='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C','ingetrazo','rev-parse','HEAD'],cwd=root,text=True).strip()==pin
assert not subprocess.check_output(['git','-C','ingetrazo','status','--porcelain'],cwd=root,text=True).strip()
env=dict(os.environ,CARGO_TARGET_DIR='/tmp/ingetrazo-native-font-merge-target',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',XDG_CONFIG_HOME=str(work/'config'),XDG_CACHE_HOME=str(work/'cache'),QT_QPA_PLATFORM='offscreen',PYTHONPATH='/work/ingetrazo')
stages=[]
def run(name,cmd,output=None):
    start=time.monotonic()
    with (work/(name+'.log')).open('w') as log:
        if output is None: result=subprocess.run(cmd,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
        else:
            with Path(output).open('w') as out: result=subprocess.run(cmd,cwd=root,env=env,stdout=out,stderr=log)
    stages.append(dict(name=name,command=cmd,source_commit=sha,exit=result.returncode,seconds=round(time.monotonic()-start,3)))
    (work/'stages.json').write_text(json.dumps(stages,indent=2)+'\n')
    print(name,result.returncode,flush=True)
    return result.returncode==0
run('workspace-tests',['cargo','test','--workspace','--locked','--tests','--no-fail-fast','-j2'])
run('doctests',['cargo','test','--workspace','--locked','--doc','--no-fail-fast','-j2'])
run('examples-check',['cargo','check','--workspace','--locked','--examples','-j2'])
names=['scale_workflows','scale_egui_input','scale_context','pushpull_closed_acquisition','native_vulkan_face_sweep_application_probe']
cmd=['cargo','build','--locked','-j2','-p','ingetrazo-gui']
for name in names: cmd+=['--example',name]
if not run('named-build',cmd):raise SystemExit('Named build failed; stale binaries will not execute')
binary=Path(env['CARGO_TARGET_DIR'])/'debug/examples'
(work/'binary-sha256.json').write_text(json.dumps({n:hashlib.sha256((binary/n).read_bytes()).hexdigest() for n in names},indent=2)+'\n')
audit=root/'docs/audits'
manifest=json.loads((audit/'scale-workflows/manifest.json').read_text())
archive=audit/'scale-workflows/records.zip'
assert hashlib.sha256(archive.read_bytes()).hexdigest()==manifest['archive_sha256']
with zipfile.ZipFile(archive) as z:
    for name in ['source.json','warp-source.json']:
        blob=z.read(name);assert hashlib.sha256(blob).hexdigest()==manifest['files'][name]
        (work/name).write_bytes(blob)
if run('scale-native',[str(binary/'scale_workflows'),str(work/'source.json'),str(work/'scale-native.json')]):
    run('scale-check',['python3','scripts/check-scale-workflows.py',str(work/'source.json'),str(work/'warp-source.json'),str(work/'scale-native.json'),str(work/'scale-comparison.json')])
shared=audit/'scale-shared/source.json'
if run('shared-native',[str(binary/'scale_workflows'),str(shared),str(work/'shared-native.json')]):
    run('shared-check',['python3','scripts/check-scale-shared.py',str(shared),str(work/'shared-native.json'),str(work/'shared-comparison.json')])
clips=audit/'scale-egui-ingress/source-eye-clip.json'
if run('egui-native',[str(binary/'scale_egui_input'),str(work/'egui-native.json'),str(clips)]):
    run('egui-check',['python3','scripts/check-scale-egui-input.py',str(clips),str(work/'egui-native.json')])
context=audit/'scale-context-lifetime/source.json'
if run('context-native',[str(binary/'scale_context'),str(context),str(work/'context-native.json')]):
    run('context-check',['python3','scripts/check-scale-context.py',str(context),str(work/'context-native.json')],work/'context-comparison.json')
closed=audit/'pushpull-endpoint-vcb/source.json'
if run('endpoint-native',[str(binary/'pushpull_closed_acquisition'),str(closed),str(work/'endpoint-native.json')]):
    run('endpoint-check',['python3','scripts/check-pushpull-closed-vcb.py',str(closed),str(work/'endpoint-native.json'),str(work/'endpoint-comparison.json')])
if run('sweep-unpack',['python3','scripts/check-metal-face-sweep.py',str(audit/'face-sweep-label-source'),str(work/'sweep-input'),'--unpack']):
    run('vulkan-app',['/tmp/ingetrazo-parity-python/bin/python','scripts/check-native-face-sweep-application.py',str(binary/'native_vulkan_face_sweep_application_probe'),str(work/'vulkan-app'),'--source-dir',str(work/'sweep-input'),'--source',sha])
# Cache and source-camera changes are jointly exercised by this freshly built app.
run('vulkan-textured-app',['/tmp/ingetrazo-parity-python/bin/python','scripts/check-native-face-sweep-application.py',str(binary/'native_vulkan_face_sweep_application_probe'),str(work/'vulkan-textured-app'),'--source-dir',str(work/'sweep-input'),'--source',sha,'--case','front-texture-back-override'])
# Hardware exact-frame checks run through gpu-replay.py in the owned NVIDIA devbox.
inputs=[shared,clips,context,closed,archive,audit/'face-sweep-label-source/records.zip']
(work/'input-sha256.json').write_text(json.dumps({str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs},indent=2)+'\n')
raise SystemExit(any(s['exit'] for s in stages))
