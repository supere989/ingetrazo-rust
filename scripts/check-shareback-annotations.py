#!/usr/bin/env python3
"""Compare actual original operative commands with the native application API."""
import argparse,json,copy
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('root',type=Path);a=p.parse_args();root=a.root
source=json.loads((root/'original.json').read_text());native=json.loads((root/'native'/'native.json').read_text());old=json.loads((root/'old-snapshot'/'native.json').read_text())
assert source['provenance']['original_python_commit']=='6be29fe437a7cd992d4e079f71720821b25216b9'
expected=source['cases'];assert set(expected)=={'finish','undo0','redo0','undo1','redo1','annotation_only_history','section_retained','nested_parked_and_root'}
assert expected['annotation_only_history']==[1,0,1] and expected['section_retained']==[1,1,1]
assert expected['finish']['dimensions'][0]['b']==[11,20,30]
assert expected['undo0']['dimensions'][1]['b']==[14,20,30] and not expected['redo1']['dimensions'][1]['anchor_b']
def verify(value):assert value['cases']==expected
verify(native)
negative=[]
assert len(old['cases']['undo0']['dimensions'])==1
assert old['cases']['finish']['dimensions'][0]['b']==[14,20,30]
try:assert old['cases']['undo0']==expected['undo0']
except AssertionError:negative.append('literal-old-scene-snapshot')
else:raise AssertionError('Old snapshot unexpectedly passed')
for name,value in [('restore-dead-anchor',copy.deepcopy(native)),('discard-added-annotation',copy.deepcopy(native)),('follow-temporary-edit-vertex',copy.deepcopy(native))]:
 if name=='restore-dead-anchor':value['cases']['redo1']['dimensions'][1]['anchor_b']=True
 if name=='discard-added-annotation':value['cases']['undo0']['dimensions'].pop()
 if name=='follow-temporary-edit-vertex':value['cases']['finish']['dimensions'][0]['b']=[14,20,30]
 try:verify(value)
 except AssertionError:negative.append(name)
 else:raise AssertionError('Negative passed '+name)
report={'status':'PASS','cases':list(expected),'negative_controls':negative,'scope':'Original Viewport/History command behavior versus Rust application API; no key dispatch/rendered UI, complete annotation family or Metal claim.'}
(root/'result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
