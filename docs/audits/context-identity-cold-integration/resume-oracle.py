import os,subprocess,time,json,shutil
from pathlib import Path
r=Path('/tmp/ingetrazo-next-integration');os.chdir(r)
os.environ.update(CARGO_TARGET_DIR='/tmp/ingetrazo-native-font-merge-target',CARGO_BUILD_JOBS='2',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',QT_QPA_PLATFORM='offscreen',PYTHONPATH='/work/ingetrazo',XDG_CONFIG_HOME='/tmp/ingetrazo-context-save-config',XDG_CACHE_HOME='/tmp/ingetrazo-context-save-cache')
out=Path('/tmp/ingetrazo-context-identity-integration');out.mkdir(exist_ok=True)
report={'runtime':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'steps':[]}
def run(name,args,allowed=(0,)):
 start=time.monotonic()
 with (out/(name+'.log')).open('w')as log:code=subprocess.run(args,stdout=log,stderr=subprocess.STDOUT).returncode
 row={'step':name,'exit':code,'seconds':round(time.monotonic()-start,3)};report['steps'].append(row);(out/'acceptance.json').write_text(json.dumps(report,indent=2));print(json.dumps(row),flush=True)
 if code not in allowed:raise SystemExit(code or 99)
# Resume source-reader invocation only after correcting its Python import path.
report=json.loads((out/'acceptance.json').read_text())
bin=Path(os.environ['CARGO_TARGET_DIR'])/'debug/examples';py='/tmp/ingetrazo-parity-python/bin/python'
manifest='/tmp/ingetrazo-igz-integrated-inputs/manifest.json'
run('igz-source-reopen',[py,'scripts/reference-igz-edit-preservation.py','inspect',manifest,str(out/'igz-native'),str(out/'igz-observed.json')])
run('igz-check',[py,'scripts/check-igz-edit-preservation.py','docs/audits/igz-edit-preservation/expected-after.json.gz',str(out/'igz-observed.json'),manifest,str(out/'igz-native')])
run('pushpull-native',[str(bin/'pushpull_gate2_fixture'),'docs/audits/pushpull-gate2/source.json',str(out/'pushpull-native.json')])
run('pushpull-live-check',[py,'scripts/check-pushpull-live-boundary.py','docs/audits/pushpull-ctrl-boundary/live-original.json',str(out/'pushpull-native.json'),str(out/'pushpull-comparison.json')])
run('pushpull-app',[str(bin/'pushpull_gate2_app_fixture'),'docs/audits/pushpull-gate2/source.json',str(out/'pushpull-app.json')])
run('pushpull-expanded-check',[py,'scripts/check-pushpull-gate2.py',str(out/'pushpull-native.json'),str(out/'pushpull-expanded.json')],(1,))
run('retained-gpu-evidence',[py,'scripts/check-texture-cold-performance.py'])
rows=json.loads((out/'pushpull-expanded.json').read_text());assert len(rows)==26 and sum(row['pass']for row in rows)==23;assert {row['name']for row in rows if not row['pass']}=={'wedge0--0.4','ctrl-roof-0.2','ctrl-step-0.5'}
report['complete']=True;(out/'acceptance.json').write_text(json.dumps(report,indent=2));print('COMBINED ACCEPTANCE COMPLETE',flush=True)
