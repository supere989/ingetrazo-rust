import os,subprocess,time,json
from pathlib import Path
root=Path('/tmp/ingetrazo-next-integration');os.chdir(root)
os.environ.update(CARGO_TARGET_DIR='/tmp/ingetrazo-native-font-merge-target',CARGO_BUILD_JOBS='2',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',QT_QPA_PLATFORM='offscreen',PYTHONPATH='/work/ingetrazo',XDG_CONFIG_HOME='/tmp/ingetrazo-combined-next-config',XDG_CACHE_HOME='/tmp/ingetrazo-combined-next-cache')
out=Path('/tmp/ingetrazo-combined-next');out.mkdir(exist_ok=True)
report={'runtime':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'steps':[]}
def run(name,args,allowed=(0,)):
 start=time.monotonic()
 with (out/(name+'.log')).open('w')as log:code=subprocess.run(args,stdout=log,stderr=subprocess.STDOUT).returncode
 row={'name':name,'seconds':round(time.monotonic()-start,3),'exit':code};report['steps'].append(row);(out/'acceptance.json').write_text(json.dumps(report,indent=2));print(json.dumps(row),flush=True)
 if code not in allowed:raise SystemExit(code or 99)
# Prior local invalidation retained; resume after the test initializer correction.
# Resume only the corrected test and the targets not reached by the failed suite.
run('corrected-group-test',['cargo','test','--locked','-p','ingetrazo-tools','--lib','base::group_visibility_tests::hidden_faces_and_placement_layers_control_closed_group_picks','--','--exact'])
run('remaining-tool-tests',['cargo','test','--locked','-p','ingetrazo-tools','--test','fillet_acceptance','--test','followme_acceptance','--test','followme_manual_acceptance','--test','pushpull_acceptance','--test','rectangle_context_axes'])
run('workspace-doctests',['cargo','test','--workspace','--locked','--doc'])
run('workspace-examples',['cargo','check','--workspace','--locked','--examples'])
run('named-build',['cargo','build','--locked','-p','ingetrazo-formats','-p','ingetrazo-tools','-p','ingetrazo-gui','--example','igz_edit_preservation','--example','export_acceptance_fixture','--example','layer_picking_fixture','--example','pushpull_gate2_app_fixture','--example','pushpull_gate2_fixture'])
bin=Path(os.environ['CARGO_TARGET_DIR'])/'debug/examples';py='/tmp/ingetrazo-parity-python/bin/python'
run('layer-native',[str(bin/'layer_picking_fixture'),'/tmp/ingetrazo-layer-original',str(out/'layer-native.json')])
layer=out/'layer';layer.mkdir(exist_ok=True)
(layer/'original.json').write_bytes((root/'docs/audits/layer-picking/original.json').read_bytes());(layer/'native.json').write_bytes((out/'layer-native.json').read_bytes())
run('layer-check',['python3','scripts/check-layer-picking.py',str(layer)])
manifest='/tmp/ingetrazo-igz-integrated-inputs/manifest.json'
run('igz-native',[str(bin/'igz_edit_preservation'),manifest,str(out/'igz-native')])
run('igz-source-reopen',[py,'scripts/reference-igz-edit-preservation.py','inspect',manifest,str(out/'igz-native'),str(out/'igz-observed.json')])
run('igz-check',[py,'scripts/check-igz-edit-preservation.py','docs/audits/igz-edit-preservation/expected-after.json.gz',str(out/'igz-observed.json'),manifest,str(out/'igz-native')])
run('pushpull-native',[str(bin/'pushpull_gate2_fixture'),'docs/audits/pushpull-gate2/source.json',str(out/'pushpull-native.json')])
run('pushpull-check',[py,'scripts/check-pushpull-gate2.py',str(out/'pushpull-native.json'),str(out/'pushpull-comparison.json')],(1,))
rows=json.loads((out/'pushpull-comparison.json').read_text());baseline=json.loads((root/'docs/audits/pushpull-gate2/final-comparison.json').read_text());assert rows==baseline,'Integrated Push/Pull results changed';assert sum(x['pass']for x in rows)==22
run('pushpull-app',[str(bin/'pushpull_gate2_app_fixture'),'docs/audits/pushpull-gate2/source.json',str(out/'pushpull-app.json')])
run('export-check',['python3','scripts/check-export-parity.py'])
report['known_pushpull_failures']=[x['name']for x in rows if not x['pass']];report['complete']=True;(out/'acceptance.json').write_text(json.dumps(report,indent=2));print('COMBINED ACCEPTANCE COMPLETE',flush=True)
