#!/usr/bin/env python3
"""Actual original embedded glyph subset against direct native intermediate paths."""
import json,sys,os,subprocess,hashlib
from pathlib import Path
source=Path(sys.argv[1]);observations=Path(sys.argv[2]);out=Path(sys.argv[3]);out.mkdir(parents=True,exist_ok=True)
subprocess.run([sys.executable,'scripts/probe-paper-text-subsets.py',source,out],check=True)
subprocess.run(['cargo','build','--locked','-p','ingetrazo-formats','--example','paper_subset_fixture'],check=True)
binary=Path(os.environ.get('CARGO_TARGET_DIR','target'))/'debug/examples/paper_subset_fixture'
for case in json.loads(observations.read_text())['cases']:
 for call in case['source_draw_calls']:
  assert hashlib.sha256(Path(call['font']['file']).read_bytes()).hexdigest()==call['font']['sha256'],'original font changed'
p=subprocess.run([binary,observations],capture_output=True,text=True);(out/'native.log').write_text(p.stderr);assert p.returncode==0
native=json.loads(p.stdout);(out/'native-subset-paths.json').write_text(p.stdout)
original=json.loads((out/'actual-original-subset-controls.json').read_text());names=['accents','bold','combining','italic','kerns','latin','mono','serif','serif_bold']
def compare(native):
 assert [c['case']for c in original['cases']]==names
 assert sorted(c['case']for c in native['cases'])==names
 count=0
 for a in original['cases']:
  b=next(c for c in native['cases']if c['case']==a['case']);assert len(a['cids_in_draw_order'])==len(b['glyphs_in_draw_order'])
  for cid,g in zip(a['cids_in_draw_order'],b['glyphs_in_draw_order']):
   f=a['font']['glyphs'][cid]
   if f.get('empty'):assert not g['points'],a['case']
   else:assert f['points']==g['points']and f['ends']==g['ends'],a['case']
   count+=1
 return count
count=compare(native)
for case in native['cases']:
 original_case=next(c for c in json.loads(observations.read_text())['cases']if c['name']==case['case'])
 assert [g['gid']for g in case['glyphs_in_draw_order']]==original_case['source_draw_calls'][0]['supplementary_qtextlayout']['runs'][0]['glyphs'],'glyph identity changed'
negative=json.loads(p.stdout);negative['cases'][0]['glyphs_in_draw_order'][0]['points'][0][0]+=1
try:compare(negative)
except AssertionError:pass
else:raise AssertionError('one native em-coordinate mutation accepted')
(out/'summary.json').write_text(json.dumps({'passed':True,'exact_cases':9,'glyph_draws':count,'negative':'one em coordinate rejected','native_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'observations_sha256':hashlib.sha256(observations.read_bytes()).hexdigest(),'scope':'Actual original rebuilt glyph points/rings vs experimental native conversion, no embedded-PDF/font-layout/UI closure'},indent=2)+'\n')
print('PASS9actualoriginalsubsetpoint/endarrays +1integercoordinate negative; PDF/layout OPEN')
