#!/usr/bin/env python3
"""Compiled actual callback-order negative; restores owned fixture source."""
import os,json,subprocess,hashlib
from pathlib import Path
r=Path(__file__).resolve().parents[1];source=r/'crates/ingetrazo-gui/examples/paper_border_native_fixture.rs';original=source.read_text();out=r/'docs/audits/paper-canvas-border/native-order-negative';out.mkdir(exist_ok=True)
env=dict(os.environ,CARGO_TARGET_DIR='/tmp/ingetrazo-geometry-paper-canvas-target',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',QT_QPA_PLATFORM='offscreen',INGETRAZO_BORDER_CASE_ONLY='sharp-solid',INGETRAZO_BORDER_EVIDENCE=str(out))
needle='painter.rect_filled(egui::Rect::from_min_size(origin,egui::vec2(w as f32*zoom,h as f32*zoom)),0.,egui::Color32::RED);\n    assert!(self.cache.paint(painter,Some(&self.factory),&self.sheet,zoom,origin).unwrap());';replacement='assert!(self.cache.paint(painter,Some(&self.factory),&self.sheet,zoom,origin).unwrap());\n    painter.rect_filled(egui::Rect::from_min_size(origin,egui::vec2(w as f32*zoom,h as f32*zoom)),0.,egui::Color32::RED);';assert original.count(needle)==1
binary=Path(env['CARGO_TARGET_DIR'])/'debug/examples/paper_border_native_fixture'
def build(name):
 with(out/(name+'.log')).open('w')as log:subprocess.run(['cargo','build','-j2','-p','ingetrazo-gui','--example','paper_border_native_fixture'],cwd=r,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
try:
 source.write_text(original.replace(needle,replacement));build('mutant-build');sha=hashlib.sha256(binary.read_bytes()).hexdigest()
 with(out/'checker.log').open('w')as log:run=subprocess.run(['/tmp/ingetrazo-parity-python/bin/python','scripts/check-paper-canvas-border-native.py'],cwd=r,env=env,stdout=log,stderr=subprocess.STDOUT)
 report=json.loads((out/'report.json').read_text());assert run.returncode==1 and len(report['cases'])==1 and report['cases'][0]['name']=='sharp-solid'and report['cases'][0]['changed']>0
 (out/'mutation.json').write_text(json.dumps(dict(binary_sha256=sha,exit=run.returncode,changed=report['cases'][0]['changed'],mutation='Move native source border callback before real red item, violating original z=1e6 order',scope='Actual callback draw-order negative; production composer caller/order remains separately source-reviewed'),indent=2)+'\n')
finally:
 source.write_text(original);build('restored-build')
