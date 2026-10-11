#!/usr/bin/env python3
"""Read-only exact recovery ownership/process/source evidence checker."""
import copy,hashlib,json,sys,zipfile
from pathlib import Path
root=Path(__file__).resolve().parents[1];audit=root/'docs/audits/session-owner-recovery';m=json.loads((audit/'manifest.json').read_text());raw=(audit/'records.zip').read_bytes();assert hashlib.sha256(raw).hexdigest()==m['archive_sha256']
with zipfile.ZipFile(audit/'records.zip') as z:
 assert set(z.namelist())==set(m['records'])
 for name,r in m['records'].items():b=z.read(name);assert len(b)==r['bytes'] and hashlib.sha256(b).hexdigest()==r['sha256'],name
 original=json.loads(z.read('source/original.json'));native=json.loads(z.read('native/result.json'));negative=json.loads(z.read('negative/result.json'))
 assert z.read('native/original-before.igz')==z.read('native/original-after.igz')
 assert hashlib.sha256(z.read('native/original-before.igz')).hexdigest()==native['original_sha256']
 recovered=json.loads(z.read('native/recovered.igz'))['scene'];assert recovered['faces']==[{'vertices':[[9.,8.,7.],[1.,0.,0.],[0.,1.,0.]]}];assert len(recovered['edges'])==3
 assert original['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9' and original['qt']=='6.12.0'
 assert {k:original[k] for k in ['pending_before','pending_after_write','pending_after_retire','recovered','discarded_recovered','discarded_copies','original_unchanged']}=={'pending_before':False,'pending_after_write':True,'pending_after_retire':False,'recovered':[9,8,7],'discarded_recovered':[9,8,7],'discarded_copies':1,'original_unchanged':True}
def check(r):
 assert r['complete'] is True and r['original_unchanged'] is True and r['abrupt_exit']!=0
 assert [s['phase'] for s in r['states']]==['live-independent-owner','live-without-metadata','clone-retains-lease','crash-visible','reused-live-pid-does-not-hide-crash','lease-observation-refusal','egui-picker-recover-close-save']
 for s,expected in zip(r['states'][:5],[0,0,0,1,1]):assert s['count']==expected and s['show_recovery']==bool(expected) and s['error'] is None
 refused=r['states'][5];assert refused['count'] is None and refused['error'] and not refused['show_recovery']
 s=r['states'][6];assert s['recovered']==[9,8,7] and s['cancel_preserved'] and s['failed_save_preserved'] and s['failed_save_status'] and s['saved_copy'] and s['close_allowed'] and s['discarded_copies']==1
 assert s['preferences']['autosave_minutes']==7 and s['preferences']['future-setting']=={'opaque':[1,2]}
 assert 'Recover an autosave' in s['picker_labels'] and 'Later' in s['picker_labels']
 assert r['writer']['pid'] not in [s['pid'] for s in r['states']]
check(native)
assert negative['states'][1]['count']==1 and negative['states'][1]['show_recovery'] is True and negative['failure']=='AssertionError()' and negative['cleanup']==['owned writer terminated']
for field,value in [('complete',False),('original_unchanged',False),('abrupt_exit',0)]:
 bad=copy.deepcopy(native);bad[field]=value
 try:check(bad)
 except AssertionError:pass
 else:raise AssertionError(field+' negative admitted')
for index in range(5):
 bad=copy.deepcopy(native);bad['states'][index]['count']=1-bad['states'][index]['count']
 try:check(bad)
 except AssertionError:pass
 else:raise AssertionError('ownership mutation admitted')
for field in ['cancel_preserved','failed_save_preserved','saved_copy','close_allowed']:
 bad=copy.deepcopy(native);bad['states'][6][field]=False
 try:check(bad)
 except AssertionError:pass
 else:raise AssertionError(field+' mutation admitted')
if len(sys.argv)>1:check(json.loads(Path(sys.argv[1]).read_text()))
print('PASS: 7 independent process/app states, original source law, exact unchanged file, compiled PID-only failure and12 negatives')
