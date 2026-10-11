#!/usr/bin/env python3
"""One combined suite and one joint build; source captures are reused unchanged."""
import hashlib,json,os,subprocess,time,zipfile
from pathlib import Path
root=Path(__file__).resolve().parents[3]
work=Path('/tmp/ingetrazo-selection-inward-integration')
work.mkdir(exist_ok=False)
env=os.environ.copy()
env.update(CARGO_TARGET_DIR='/tmp/ingetrazo-native-font-merge-target',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',XDG_CONFIG_HOME=str(work/'config'),XDG_CACHE_HOME=str(work/'cache'),QT_QPA_PLATFORM='offscreen',PYTHONPATH='/work/ingetrazo')
sha=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
assert subprocess.check_output(['git','-C','ingetrazo','rev-parse','HEAD'],cwd=root,text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9'
assert not subprocess.check_output(['git','-C','ingetrazo','status','--porcelain'],cwd=root,text=True).strip()
stages=[]
def run(name,cmd):
 start=time.monotonic()
 with (work/(name+'.log')).open('w') as log:
  result=subprocess.run(cmd,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
 stages.append(dict(name=name,command=cmd,source_commit=sha,exit=result.returncode,seconds=round(time.monotonic()-start,3)))
 (work/'stages.json').write_text(json.dumps(stages,indent=2)+'\n')
 print(name,result.returncode,flush=True)
 return result.returncode==0
run('workspace-tests',['cargo','test','--workspace','--locked','--tests','--no-fail-fast','-j2'])
run('doctests',['cargo','test','--workspace','--locked','--doc','--no-fail-fast','-j2'])
run('examples-check',['cargo','check','--workspace','--locked','--examples','-j2'])
names=['pushpull_inward_limits','select_visible_click_protocol','pushpull_partition_collapse','pushpull_endpoints_app']
cmd=['cargo','build','--locked','-j2','-p','ingetrazo-gui']
for name in names:cmd+=['--example',name]
cmd+=['-p','ingetrazo-tools','--example','pushpull_gate2_fixture']
if not run('named-build',cmd):raise SystemExit('Named build failed; stale binaries will not execute')
bin=Path(env['CARGO_TARGET_DIR'])/'debug/examples'
(work/'binary-sha256.json').write_text(json.dumps({name:hashlib.sha256((bin/name).read_bytes()).hexdigest() for name in names+['pushpull_gate2_fixture']},indent=2)+'\n')
audit=root/'docs/audits'
inward=audit/'pushpull-inward-limits'
if run('inward-native',[str(bin/'pushpull_inward_limits'),str(inward/'source.json'),str(work/'inward-native.json')]):
 run('inward-check',['python3','scripts/check-pushpull-inward-limits.py',str(inward/'source.json'),str(work/'inward-native.json'),str(inward/'source-vcb.json'),str(inward/'remaining-native.json'),str(work/'inward-comparison.json')])
select=audit/'select-visible-click'
manifest=json.loads((select/'manifest.json').read_text())
archive=select/manifest['archive']
assert hashlib.sha256(archive.read_bytes()).hexdigest()==manifest['archive_sha256']
with zipfile.ZipFile(archive) as z:
 data=z.read('original.json')
 assert hashlib.sha256(data).hexdigest()==manifest['files']['original.json']
 (work/'select-original.json').write_bytes(data)
if run('select-native',[str(bin/'select_visible_click_protocol'),str(work/'select-original.json'),str(work/'select-native.json')]):
 run('select-check',['python3','scripts/check-select-visible-click.py',str(select),str(work/'select-native.json')])
partition=audit/'pushpull-partition-collapse/source.json'
if run('partition-native',[str(bin/'pushpull_partition_collapse'),str(partition),str(work/'partition-native.json')]):
 run('partition-check',['python3','scripts/check-pushpull-partition-collapse.py',str(partition),str(work/'partition-native.json'),str(work/'partition-comparison.json')])
endpoint=audit/'pushpull-endpoints/source.json'
primitive=run('endpoint-native',[str(bin/'pushpull_gate2_fixture'),str(endpoint),str(work/'endpoint-native.json')])
app=run('endpoint-app',[str(bin/'pushpull_endpoints_app'),str(endpoint),str(work/'endpoint-app.json')])
if primitive and app:
 run('endpoint-check',['python3','scripts/check-pushpull-endpoints.py',str(endpoint),str(work/'endpoint-native.json'),str(work/'endpoint-comparison.json'),str(work/'endpoint-app.json')])
raise SystemExit(any(stage['exit'] for stage in stages))
