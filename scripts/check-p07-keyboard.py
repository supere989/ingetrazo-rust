#!/usr/bin/env python3
"""Strict missing-keyboard source-state comparison, including actual egui routing."""
import json,sys,math,copy
from pathlib import Path
NAMES=['duplicate-comma','duplicate-dot','mixed-decimal','arabic-decimal-glyph','superscript-invalid','unsupported-letter','double-minus','fraction-prefix','unit-mid-token','rectangle-comma-list','rectangle-second-comma','backspace-unicode','ctrl-backspace','zero-denominator','chords-and-altgr','escape-and-repeat','unit-empty-shortcut','space-empty-select','array-prefix-edit']
def check(source,native):
 assert source['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [r['recipe']['name'] for r in source['cases']]==NAMES
 assert [r['name'] for r in native['cases']]==NAMES
 mismatches=[];count=0
 for s,n in zip(source['cases'],native['cases']):
  assert len(s['states'])==len(n['states'])==len(s['recipe']['operations'])+1
  for x,y in zip(s['states'],n['states']):
   assert x['phase']==y['phase']
   a=y['state'];assert type(a['buffer'])is str and type(a['tool'])is str
   assert type(a['faces'])is int and a['faces']>=0
   assert all(type(v)is int and v>=0 for v in a['history'])and len(a['history'])==2
   for point in a['vertices']+[p for e in a['edges']for p in e]+([]if a['anchor']is None else[a['anchor']]):
    assert len(point)==3 and all(type(v)in(int,float)and math.isfinite(v)for v in point)
   count+=1
   if x['state']!=a:mismatches.append({'case':n['name'],'phase':x['phase'],'source':x['state'],'native':a})
 assert type(native["payload_guards"]["editor_event_retained"]) is bool
 assert native["payload_guards"]=={"mixed":"","digits":"12","modal":"","editor_buffer":"","editor_event_retained":True}
 assert native["one_frame_chords"]==source["cases"][14]["states"][-1]["state"]
 return count,mismatches
if __name__=='__main__':
 source=json.loads(Path(sys.argv[1]).read_text());native=json.loads(Path(sys.argv[2]).read_text());count,errors=check(source,native)
 controls=[]
 if not errors:
  def reject(name,mutate):
   bad=copy.deepcopy(native);mutate(bad)
   try:_,e=check(source,bad);okay=bool(e)
   except(AssertionError,KeyError,TypeError):okay=True
   assert okay,name;controls.append(name)
  reject('single-frame-chord-mutated',lambda d:d['one_frame_chords'].__setitem__('buffer','1'))
  reject('truncated-roster',lambda d:d['cases'].pop())
  reject('duplicate-decimal-accepted',lambda d:d['cases'][0]['states'][4]['state'].__setitem__('buffer','2,5,'))
  reject('unsupported-arabic-decimal',lambda d:d['cases'][3]['states'][2]['state'].__setitem__('buffer','١٫'))
  reject('escape-cancelled-anchor',lambda d:d['cases'][15]['states'][2]['state'].__setitem__('anchor',None))
  reject('repeat-stayed-select',lambda d:d['cases'][15]['states'][5]['state'].__setitem__('tool','select'))
  reject('wrong-type-face-count',lambda d:d['cases'][0]['states'][0]['state'].__setitem__('faces',False))
  reject('lost-authored-position',lambda d:d['cases'][0]['states'][-1]['state']['vertices'][0].__setitem__(0,9))
 report={'passed':not errors,'cases':19,'states':count,'mismatches':errors,'negative_controls':controls,'scope':'Actual egui App keyboard ingress versus original viewport buffer/handler and MainWindow activation; no OS shortcut delivery or whole-frame claim.'}
 Path(sys.argv[3]).write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n');print(json.dumps({'passed':not errors,'states':count,'mismatches':len(errors),'negative_controls':len(controls)}));sys.exit(bool(errors))
