#!/usr/bin/env python3
"""One stable integrated suite with explicit stop/resume before its joint consumer build."""
import subprocess,os,json,time,shutil,sys,argparse
from pathlib import Path
root=Path(__file__).resolve().parents[3];audit=Path(__file__).resolve().parent
work=Path('/tmp/ingetrazo-visibility-ghost-partition-integration');work.mkdir(exist_ok=True)
env=os.environ.copy();env.update(CARGO_TARGET_DIR='/tmp/ingetrazo-native-font-merge-target',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',XDG_CONFIG_HOME=str(work/'config'),XDG_CACHE_HOME=str(work/'cache'),QT_QPA_PLATFORM='offscreen',PYTHONPATH='/work/ingetrazo')
source_pin=subprocess.check_output(['git','-C','ingetrazo','rev-parse','HEAD'],cwd=root,text=True).strip()
assert source_pin=='6be29fe437a7cd992d4e079f71720821b25216b9'
assert not subprocess.check_output(['git','-C','ingetrazo','status','--porcelain'],cwd=root,text=True).strip()
parser=argparse.ArgumentParser()
parser.add_argument('--start-at',choices=['named-build'])
parser.add_argument('--stop-after',choices=['examples-check'])
args=parser.parse_args()
stages=json.loads((work/'stages.json').read_text()) if args.start_at else []
running=args.start_at is None
def run(name,command,expected=0):
 global running
 if not running:
  if name!=args.start_at:return
  running=True
 source_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
 start=time.monotonic()
 with (work/(name+'.log')).open('w')as log:r=subprocess.run(command,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
 stages.append({'source_commit':source_commit,'name':name,'command':command,'exit':r.returncode,'seconds':round(time.monotonic()-start,3)})
 (work/'stages.json').write_text(json.dumps(stages,indent=2))
 if r.returncode!=expected:raise SystemExit(name+' failed: '+str(r.returncode))
 if name==args.stop_after:raise SystemExit(0)
run('workspace-tests',['cargo','test','--workspace','--locked','--tests','-j2'])
run('doctests',['cargo','test','--workspace','--locked','--doc','-j2'])
run('examples-check',['cargo','check','--workspace','--locked','--examples','-j2'])
run('named-build',['cargo','build','--locked','-j2','-p','ingetrazo-gui','--example','context_document_lifecycle','--example','line_command_history_fixture','--example','pushpull_partition_collapse','--example','unhide_actions_fixture','--example','native_vulkan_texture_draw_probe','--example','native_vulkan_hidden_ghost_application_probe','--example','saved_view_history_fixture','--example','pushpull_endpoints_app','-p','ingetrazo-tools','--example','pushpull_gate2_fixture','-p','ingetrazo-formats','--example','component_acceptance_fixture','--example','component_editor_acceptance_fixture','--example','component_ci_world_bake_fixture'])
bin=Path(env['CARGO_TARGET_DIR'])/'debug/examples'
lifecycle=work/'lifecycle';lifecycle.mkdir(exist_ok=True)
for name in ['input.igz','replacement.igz','corrupt.igz','original.json','original-pending.json']:
 shutil.copyfile(Path('/tmp/ingetrazo-document-lifecycle-proof')/name,lifecycle/name)
run('lifecycle-native',[str(bin/'context_document_lifecycle'),str(lifecycle)])
run('lifecycle-original-reopen',['/tmp/ingetrazo-parity-python/bin/python','scripts/reference-context-document-lifecycle.py',str(lifecycle),'inspect'])
run('lifecycle-strict-check',['python3','scripts/check-context-document-lifecycle.py',str(lifecycle)])
line=work/'line';line.mkdir(exist_ok=True)
for path in (Path('/tmp/ingetrazo-line-orphan-history-proof')/'commands').iterdir():
 if path.is_file() and path.name not in ('native.json',):shutil.copyfile(path,line/path.name)
run('line-native',[str(bin/'line_command_history_fixture'),str(line)])
shutil.copyfile(work/'line-native.log',line/'native.json')
run('line-check',['python3','scripts/check-line-command-history.py',str(line)])
partition=work/'partition';partition.mkdir(exist_ok=True)
shutil.copyfile(root/'docs/audits/pushpull-partition-collapse/source.json',partition/'source.json')
run('partition-native',[str(bin/'pushpull_partition_collapse'),str(partition/'source.json'),str(partition/'native.json')])
run('partition-check',['python3','scripts/check-pushpull-partition-collapse.py',str(partition/'source.json'),str(partition/'native.json'),str(partition/'comparison.json')])
views=work/'views'
for case in ['unique','duplicate']:
 output=views/case;output.mkdir(parents=True,exist_ok=True)
 for name in ['input.igz','original.json']:shutil.copyfile(Path('/tmp/ingetrazo-saved-view-history-proof')/case/name,output/name)
 run('views-'+case,[str(bin/'saved_view_history_fixture'),str(output)])
 shutil.copyfile(work/('views-'+case+'.log'),output/'native.json')
for name in ['baseline-native.json','baseline-original.json']:
 shutil.copyfile(Path('/tmp/ingetrazo-saved-view-history-proof')/name,views/name)
run('views-check',['python3','scripts/check-saved-view-history.py',str(views)])
unhide=work/'unhide';unhide.mkdir(exist_ok=True)
for name in ['original.json','original-nested.json']:
 shutil.copyfile(Path('/tmp/ingetrazo-unhide-actions-proof')/name,unhide/name)
run('unhide-native',[str(bin/'unhide_actions_fixture'),str(unhide/'native.json'),str(unhide/'native-nested.json')])
run('unhide-check',['python3','scripts/check-unhide-actions.py',str(unhide)])
run('mip-readonly-check',['/tmp/ingetrazo-parity-python/bin/python','scripts/check-texture-mip-barrier-batch.py'])
endpoints=work/'endpoints';endpoints.mkdir(exist_ok=True)
shutil.copyfile(root/'docs/audits/pushpull-endpoints/source.json',endpoints/'source.json')
run('endpoint-primitive',[str(bin/'pushpull_gate2_fixture'),str(endpoints/'source.json'),str(endpoints/'native.json')])
run('endpoint-app',[str(bin/'pushpull_endpoints_app'),str(endpoints/'source.json'),str(endpoints/'app.json')])
run('endpoint-check',['/tmp/ingetrazo-parity-python/bin/python','scripts/check-pushpull-endpoints.py',str(endpoints/'source.json'),str(endpoints/'native.json'),str(endpoints/'comparison.json'),str(endpoints/'app.json')])
run('component-ci-check',['/tmp/ingetrazo-parity-python/bin/python','scripts/check-python-components.py','--prebuilt-dir',str(bin)])
