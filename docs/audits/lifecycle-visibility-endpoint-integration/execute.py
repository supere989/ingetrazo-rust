#!/usr/bin/env python3
"""One stable integrated suite; each completed stage records its own exit/time."""
import subprocess,os,json,time,shutil,sys
from pathlib import Path
root=Path(__file__).resolve().parents[3];audit=Path(__file__).resolve().parent
work=Path('/tmp/ingetrazo-lifecycle-visibility-integration');work.mkdir(exist_ok=True)
env=os.environ.copy();env.update(CARGO_TARGET_DIR='/tmp/ingetrazo-native-font-merge-target',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',XDG_CONFIG_HOME=str(work/'config'),XDG_CACHE_HOME=str(work/'cache'),QT_QPA_PLATFORM='offscreen',PYTHONPATH='/work/ingetrazo')
resume_camera='--resume-camera' in sys.argv
resume_recovery='--resume-recovery' in sys.argv
resume=resume_camera or resume_recovery
stages=json.loads((work/'stages.json').read_text()) if resume else []
def run(name,args,expected=0):
 start=time.monotonic()
 with (work/(name+'.log')).open('w')as log:r=subprocess.run(args,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
 stages.append({'name':name,'command':args,'exit':r.returncode,'seconds':round(time.monotonic()-start,3)})
 (work/'stages.json').write_text(json.dumps(stages,indent=2))
 if r.returncode!=expected:raise SystemExit(name+' failed: '+str(r.returncode))
if resume_recovery:
 run('recovery-test-correction',['cargo','test','--locked','-j2','-p','ingetrazo-gui','--test','recovery_acceptance','crash_restart_retains_original_and_failed_close'])
elif resume_camera:
 run('camera-test-correction',['cargo','test','--locked','-j2','-p','ingetrazo-gui','--lib','document_camera_tests::'])
 run('gui-unfinished',['cargo','test','--locked','-j2','-p','ingetrazo-gui','--bins','--test','recovery_acceptance','--test','offset_controller_acceptance','--test','move_component_arrays','--test','composer_pdf_export','--test','component_global_history','--test','clipboard_transport_acceptance'])
else:
 run('local-package-handoff',['cargo','clean','-p','ingetrazo-core','-p','ingetrazo-formats','-p','ingetrazo-tools','-p','ingetrazo-render','-p','ingetrazo-gui','-p','ingetrazo-cli','-p','ingetrazo-metal'])
 run('workspace-tests',['cargo','test','--workspace','--locked','--tests','-j2'])
if resume:
 run('remaining-package-tests',['cargo','test','--locked','-j2','-p','ingetrazo-metal','-p','ingetrazo-render','-p','ingetrazo-tools','--tests'])
run('doctests',['cargo','test','--workspace','--locked','--doc','-j2'])
run('examples-check',['cargo','check','--workspace','--locked','--examples','-j2'])
run('named-build',['cargo','build','--locked','-j2','-p','ingetrazo-gui','--example','context_document_lifecycle','--example','saved_view_history_fixture','--example','pushpull_endpoints_app','-p','ingetrazo-tools','--example','pushpull_gate2_fixture','-p','ingetrazo-formats','--example','component_acceptance_fixture','--example','component_editor_acceptance_fixture','--example','component_ci_world_bake_fixture'])
bin=Path(env['CARGO_TARGET_DIR'])/'debug/examples'
lifecycle=work/'lifecycle';lifecycle.mkdir(exist_ok=True)
for name in ['input.igz','replacement.igz','corrupt.igz','original.json','original-pending.json']:
 shutil.copyfile(Path('/tmp/ingetrazo-document-lifecycle-proof')/name,lifecycle/name)
run('lifecycle-native',[str(bin/'context_document_lifecycle'),str(lifecycle)])
run('lifecycle-original-reopen',['/tmp/ingetrazo-parity-python/bin/python','scripts/reference-context-document-lifecycle.py',str(lifecycle),'inspect'])
run('lifecycle-strict-check',['python3','scripts/check-context-document-lifecycle.py',str(lifecycle)],expected=1)
report=json.loads((work/'lifecycle-strict-check.log').read_text());assert report['lifecycle_passed'] and report['strict_failures']==['pending/rows/0/phases/2/state/root_mesh/points/length','pending/rows/1/phases/2/state/root_mesh/points/length']
views=work/'views'
for case in ['unique','duplicate']:
 output=views/case;output.mkdir(parents=True,exist_ok=True)
 for name in ['input.igz','original.json']:shutil.copyfile(Path('/tmp/ingetrazo-saved-view-history-proof')/case/name,output/name)
 run('views-'+case,[str(bin/'saved_view_history_fixture'),str(output)])
 shutil.copyfile(work/('views-'+case+'.log'),output/'native.json')
for name in ['baseline-native.json','baseline-original.json']:
 shutil.copyfile(Path('/tmp/ingetrazo-saved-view-history-proof')/name,views/name)
run('views-check',['python3','scripts/check-saved-view-history.py',str(views)])
run('mip-readonly-check',['/tmp/ingetrazo-parity-python/bin/python','scripts/check-texture-mip-barrier-batch.py'])
endpoints=work/'endpoints';endpoints.mkdir(exist_ok=True)
shutil.copyfile(root/'docs/audits/pushpull-endpoints/source.json',endpoints/'source.json')
run('endpoint-primitive',[str(bin/'pushpull_gate2_fixture'),str(endpoints/'source.json'),str(endpoints/'native.json')])
run('endpoint-app',[str(bin/'pushpull_endpoints_app'),str(endpoints/'source.json'),str(endpoints/'app.json')])
run('endpoint-check',['/tmp/ingetrazo-parity-python/bin/python','scripts/check-pushpull-endpoints.py',str(endpoints/'source.json'),str(endpoints/'native.json'),str(endpoints/'comparison.json'),str(endpoints/'app.json')])
run('component-ci-check',['/tmp/ingetrazo-parity-python/bin/python','scripts/check-python-components.py','--prebuilt-dir',str(bin)])
