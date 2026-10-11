import os,subprocess,time,json,shutil
from pathlib import Path
r=Path('/tmp/ingetrazo-next-integration');os.chdir(r)
os.environ.update(CARGO_TARGET_DIR='/tmp/ingetrazo-native-font-merge-target',CARGO_BUILD_JOBS='2',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',QT_QPA_PLATFORM='offscreen',XDG_CONFIG_HOME='/tmp/ingetrazo-context-save-config',XDG_CACHE_HOME='/tmp/ingetrazo-context-save-cache')
out=Path('/tmp/ingetrazo-context-identity-integration');out.mkdir(exist_ok=True)
report={'runtime':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'steps':[]}
def run(name,args,allowed=(0,)):
 start=time.monotonic()
 with (out/(name+'.log')).open('w')as log:code=subprocess.run(args,stdout=log,stderr=subprocess.STDOUT).returncode
 row={'step':name,'exit':code,'seconds':round(time.monotonic()-start,3)};report['steps'].append(row);(out/'acceptance.json').write_text(json.dumps(report,indent=2));print(json.dumps(row),flush=True)
 if code not in allowed:raise SystemExit(code or 99)
# Resume after correcting the two IGZ no-op failures. Prior completed targets
# are retained; only corrected native preservation and unreached targets run.
report=json.loads((out/'acceptance.json').read_text())
report['corrected_runtime']=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
run('corrected-native-preservation',['cargo','test','--locked','-p','ingetrazo-formats','--test','native_preservation'])
run('remaining-format-targets',['cargo','test','--locked','-p','ingetrazo-formats','--test','preservation_acceptance','--test','roundtrips','--test','sheet_pdf','--test','skp_import'])
run('remaining-workspace-targets',['cargo','test','--locked','-p','ingetrazo-gui','-p','ingetrazo-metal','-p','ingetrazo-render','-p','ingetrazo-tools','--lib','--tests'])
run('workspace-doctests',['cargo','test','--workspace','--locked','--doc'])
run('workspace-examples',['cargo','check','--workspace','--locked','--examples'])
run('named-build',['cargo','build','--locked','-p','ingetrazo-formats','-p','ingetrazo-tools','-p','ingetrazo-gui','--example','context_document_save','--example','saved_view_identity_fixture','--example','igz_edit_preservation','--example','pushpull_gate2_fixture','--example','pushpull_gate2_app_fixture'])
bin=Path(os.environ['CARGO_TARGET_DIR'])/'debug/examples';py='/tmp/ingetrazo-parity-python/bin/python'
save=out/'context';save.mkdir(exist_ok=True)
for name in ['input.igz','original.json']:shutil.copy2(Path('/tmp/ingetrazo-context-save-proof')/name,save/name)
run('context-native',[str(bin/'context_document_save'),str(save)])
run('context-original-reopen',[py,'scripts/reference-context-document-save.py',str(save),'inspect'])
run('context-check',[py,'scripts/check-context-document-save.py',str(save)])
views=out/'views';shutil.copytree('/tmp/ingetrazo-saved-view-identity-proof',views,dirs_exist_ok=True)
# This fixture writes stdout; keep its machine report separate from invocation log.
start=time.monotonic()
with (views/'native.json').open('w')as f:subprocess.run([str(bin/'saved_view_identity_fixture'),str(views)],stdout=f,check=True)
run('view-check',[py,'scripts/check-saved-view-identities.py',str(views)])
manifest='/tmp/ingetrazo-igz-integrated-inputs/manifest.json'
run('igz-native',[str(bin/'igz_edit_preservation'),manifest,str(out/'igz-native')])
run('igz-source-reopen',[py,'scripts/reference-igz-edit-preservation.py','inspect',manifest,str(out/'igz-native'),str(out/'igz-observed.json')])
run('igz-check',[py,'scripts/check-igz-edit-preservation.py','docs/audits/igz-edit-preservation/expected-after.json.gz',str(out/'igz-observed.json'),manifest,str(out/'igz-native')])
run('pushpull-native',[str(bin/'pushpull_gate2_fixture'),'docs/audits/pushpull-gate2/source.json',str(out/'pushpull-native.json')])
run('pushpull-live-check',[py,'scripts/check-pushpull-live-boundary.py','docs/audits/pushpull-ctrl-boundary/live-original.json',str(out/'pushpull-native.json'),str(out/'pushpull-comparison.json')])
run('pushpull-app',[str(bin/'pushpull_gate2_app_fixture'),'docs/audits/pushpull-gate2/source.json',str(out/'pushpull-app.json')])
run('pushpull-expanded-check',[py,'scripts/check-pushpull-gate2.py',str(out/'pushpull-native.json'),str(out/'pushpull-expanded.json')],(1,))
run('retained-gpu-evidence',[py,'scripts/check-texture-cold-performance.py'])
report['complete']=True;(out/'acceptance.json').write_text(json.dumps(report,indent=2));print('COMBINED ACCEPTANCE COMPLETE',flush=True)
