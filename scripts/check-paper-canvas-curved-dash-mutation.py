#!/usr/bin/env python3
"""Compile wrong missing-margin production painter; actual GPU checker must reject."""
import os,subprocess,json,hashlib
from pathlib import Path
root=Path(__file__).resolve().parents[1];source=root/'crates/ingetrazo-gui/src/composer_border_dash.rs';original=source.read_text();out=root/'docs/audits/paper-canvas-curved-dash/unscaled-flatten-negative';out.mkdir(parents=True,exist_ok=True);env=dict(os.environ,INGETRAZO_BORDER_EVIDENCE=str(out),QT_QPA_PLATFORM='offscreen',CARGO_PROFILE_DEV_DEBUG='0',CARGO_INCREMENTAL='0');target=Path(env['CARGO_TARGET_DIR']);binary=target/'debug/examples/paper_curved_border_native_fixture'
def build(log):
 with log.open('w')as f:subprocess.run(['cargo','build','-p','ingetrazo-gui','--example','paper_curved_border_native_fixture','-j2'],cwd=root,env=env,stdout=f,stderr=f,check=True)
try:
 needle='0.5/density';assert original.count(needle)==1;source.write_text(original.replace(needle,'0.5'))
 build(out/'build.log')
 digest=hashlib.sha256(binary.read_bytes()).hexdigest()
 with(out/'checker.log').open('w')as f:r=subprocess.run([os.environ.get('INGETRAZO_PARITY_PYTHON','/tmp/ingetrazo-parity-python/bin/python'),str(root/'scripts/check-paper-canvas-curved-dash-native.py')],cwd=root,env=env,stdout=f,stderr=f)
 report=json.loads((out/'report.json').read_text());assert r.returncode==1;assert [c['name']for c in report['cases']]==['production-curved-dash','production-clamped-dash','production-thin-curved-dash','production-carry-dash'];assert report['cases'][0]['changed']>0
 (out/'mutation.json').write_text(json.dumps({'binary_sha256':digest,'expected_exit':r.returncode,'changed':[c['changed']for c in report['cases']],'mutation':'Remove source transform-scaled dash curve threshold; exact actual source retained'},indent=2)+'\n')
finally:
 source.write_text(original);build(out/'restored-build.log')
