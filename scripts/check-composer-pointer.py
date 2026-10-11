#!/usr/bin/env python3
"""Strict source Qt pointer versus native egui composer transactions."""
import json,sys,math,copy
from pathlib import Path
NAMES=['unselected-drag','selected-drag','large-first-move','click-only','locked','hidden','locked-overlay','hidden-overlay','z-order','equal-z-order','escape-during-drag','escape-before-move','out-and-back']
def check(source,native):
 assert source['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [r['recipe']['name']for r in source['cases']]==NAMES
 assert [r['name']for r in native['cases']]==NAMES
 assert any(t.get('handler')=='item-press'for t in source['trace'])
 assert all(t['spontaneous']and t['mode']=='select'and t['interactive']and not t['view_edit']for t in source['trace']if t.get('handler')=='view-press')
 errors=[];count=0
 for s,n in zip(source['cases'],native['cases']):
  assert len(s['states'])==len(n['states'])==len(s['recipe']['events'])+3
  assert type(n['idle_unchanged'])is bool and n['idle_unchanged']
  assert type(n['snapshots'])is int and n['snapshots']==(0 if n['name']in['click-only','locked','hidden','escape-before-move']else 1)
  assert len(n['diagnostics'])==len(s['recipe']['events'])
  assert n['diagnostics'][0]['pending']is False and n['diagnostics'][0]['snapshots']==0
  assert n['diagnostics'][-1]['pending']is False and n['diagnostics'][-1]['grab']is None
  for a,b in zip(s['states'],n['states']):
   assert a['phase']==b['phase']
   v=b['state'];assert type(v['undo'])is int and type(v['redo'])is int and v['undo']>=0 and v['redo']>=0
   assert type(v['selected'])is list and all(type(x)is str for x in v['selected'])
   for f in v['frames']:
    assert type(f['uid'])is str and type(f['locked'])is bool and type(f['hidden'])is bool
    assert len(f['rect'])==4 and all(type(x)in(int,float)and math.isfinite(x)for x in f['rect'])
    assert type(f['z'])in(int,float)and math.isfinite(f['z'])
   if a['state']!=v:errors.append({'case':n['name'],'phase':a['phase'],'source':a['state'],'native':v})
   count+=1
 assert native["cases"][0]["boundary_controls"]=={"blank_press_kept_selection":True,"blank_release_cleared_selection":True,"outside_paper_acquired":True,"outside_paper_position":[-16.,43.],"foreground_layer_owned_press":True,"foreground_preserved_item":True,"lost_capture_finished":True,"lost_capture_undo":True,"two_packet_histories":True,"two_packet_final":[-6.,48.],"two_packet_first":[-12.,45.],"two_packet_before":[-16.,43.]}
 assert all(type(v)is bool for k,v in native["cases"][0]["boundary_controls"].items()if k not in["outside_paper_position","two_packet_final","two_packet_first","two_packet_before"])
 assert count==89
 return count,errors
if __name__=='__main__':
 source=json.loads(Path(sys.argv[1]).read_text());native=json.loads(Path(sys.argv[2]).read_text());count,errors=check(source,native);controls=[]
 if not errors:
  def reject(name,fn):
   bad=copy.deepcopy(native);fn(bad)
   try:_,errors=check(source,bad);rejected=bool(errors)
   except(AssertionError,KeyError,TypeError):rejected=True
   assert rejected,name;controls.append(name)
  reject('foreground-took-underlying-item',lambda d:d['cases'][0]['boundary_controls'].__setitem__('foreground_preserved_item',False))
  reject('lost-capture-not-undoable',lambda d:d['cases'][0]['boundary_controls'].__setitem__('lost_capture_undo',False))
  reject('outside-paper-position-drift',lambda d:d['cases'][0]['boundary_controls'].__setitem__('outside_paper_position',[-15.,43.]))
  reject('coalesced-packet-transactions',lambda d:d['cases'][0]['boundary_controls'].__setitem__('two_packet_histories',False))
  reject('missing-intermediate-packet-undo',lambda d:d['cases'][0]['boundary_controls'].__setitem__('two_packet_first',[-16.,43.]))
  reject('truncated-roster',lambda d:d['cases'].pop())
  reject('unselected-drag-inert',lambda d:d['cases'][0]['states'][2]['state']['frames'][0]['rect'].__setitem__(0,40.))
  reject('press-origin-drift',lambda d:d['cases'][0]['states'][3]['state']['frames'][0]['rect'].__setitem__(0,69.))
  reject('press-selection-deferred',lambda d:d['cases'][0]['states'][1]['state'].__setitem__('selected',[]))
  reject('z-order-wrong-winner',lambda d:d['cases'][8]['states'][1]['state'].__setitem__('selected',['base']))
  reject('hidden-object-moved',lambda d:d['cases'][5]['states'][2]['state']['frames'][0]['rect'].__setitem__(0,44.))
  reject('escape-incorrect-rollback',lambda d:d['cases'][10]['states'][4]['state']['frames'][0]['rect'].__setitem__(0,40.))
  reject('missing-release-history',lambda d:d['cases'][0]['states'][4]['state'].__setitem__('undo',0))
  reject('wrong-type-lock',lambda d:d['cases'][0]['states'][0]['state']['frames'][0].__setitem__('locked',0))
  reject('eager-press-snapshot',lambda d:d['cases'][0]['diagnostics'][0].__setitem__('pending',True))
 report={'passed':not errors,'recipes':13,'states':count,'mismatches':errors,'negative_controls':controls,'scope':'Actual QtTest viewport and native App egui pointer transactions; source model drawing omitted, no pixel/frame-rate/multiitem/resize proof.'}
 Path(sys.argv[3]).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'passed':not errors,'states':count,'mismatches':len(errors),'negatives':len(controls)}));sys.exit(bool(errors))
