#!/usr/bin/env python3
"""Literal source legend authoring and separately bound corrected native gesture history."""
import argparse,copy,json,math,zipfile
from pathlib import Path
KEYS=['case','phase','composition','layers','selection','undo','redo','mode','pending','heights']
NAMES=['lifecycle','empty-visible-cancel','loaded-locked','single-legend-gestures']
def equal(a,b,p='$'):
 assert type(a)is type(b),(p,'type',type(a).__name__,type(b).__name__)
 if isinstance(a,dict):
  assert a.keys()==b.keys(),(p,'keys',list(a),list(b))
  for k in a:equal(a[k],b[k],p+'.'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),(p,'length',len(a),len(b))
  for i,(x,y)in enumerate(zip(a,b)):equal(x,y,p+'.'+str(i))
 else:
  if isinstance(a,float):assert math.isfinite(a)and math.isfinite(b),(p,'nonfinite')
  assert a==b,(p,a,b)
def validate(s,n,readers,sr,corrected):
 assert [c['name']for c in s['cases']]==NAMES and [c['name']for c in n['cases']]==NAMES
 inventory=[(r['case'],r['phase'])for r in s['states']];assert len(inventory)==69 and len(set(inventory))==69
 assert [(r['case'],r['phase'])for r in n['states']]==inventory
 assert len(corrected)==16;assert [r['phase']for r in corrected]==[r['phase']for r in s['states']if r['case']=='single-legend-gestures']
 expected={(r['case'],r['phase']):r for r in corrected}
 for a,b in zip(s['states'],n['states']):
  a=expected[(a['case'],a['phase'])]if a['case']=='single-legend-gestures'else a
  equal({k:a[k]for k in KEYS},{k:b[k]for k in KEYS})
 assert [r['case']for r in readers]==NAMES and [r['case']for r in sr]==NAMES;equal(sr,readers)
 for k in ['actual_egui','unknown_metadata','source_failed_geometry_history']:assert n[k]is True,k
 for k in ['atomic_refusal','warm_preview']:assert n['controls'][k]is True,k

def guard_values(doc,guards):
 for guard in guards:
  for path,expected in guard['paths'].items():
   v=doc
   for k in path.split('.'):v=v[int(k)]if isinstance(v,list)else v[k]
   equal(expected,v,'raw.'+path)
p=argparse.ArgumentParser();p.add_argument('source',type=Path);p.add_argument('native',type=Path);p.add_argument('comparison',type=Path);a=p.parse_args();s=json.loads((a.source/'original-legend.json').read_text());n=json.loads((a.native/'native-legend.json').read_text());r=json.loads((a.native/'native-reopens.json').read_text());sr=json.loads((a.source/'original-reopens.json').read_text());corrected=json.loads((a.source/'native-gesture-expectations.json').read_text())['states'];a.comparison.parent.mkdir(parents=True,exist_ok=True)
try:
 validate(s,n,r,sr,corrected)
 errors=s['callback_errors'];assert len(errors)==3 and [e['phase']for e in errors]==['move-release','resize-release','minimum-release']
 assert all(e['type']=='AttributeError'and "property 'h_mm'"in e['message']and 'has no setter'in e['message']for e in errors)
 assert all(x['undo']==0 and x['redo']==0 for x in s['states']if x['case']=='single-legend-gestures')
 first=json.loads((a.source/'first-gesture-hard-failure.partial.json').read_text());assert len(first['states'])==4;equal(first['states'],[x for x in s['states']if x['case']=='single-legend-gestures'][:4])
 guard=s['opaque_guards'];assert len(guard)==1 and guard[0]['case']=='loaded-locked';case=next(c for c in n['cases']if c['name']=='loaded-locked');path=(a.native/case['saved']).resolve();assert path.is_relative_to(a.native.resolve());data=path.read_bytes();raw=json.loads(data)if data[:2]!=b'PK'else json.loads(zipfile.ZipFile(path).read('document.json'));guard_values(raw,guard)
except (AssertionError,KeyError,TypeError,IndexError)as error:
 a.comparison.write_text(json.dumps(dict(result='fail',stage='baseline',error=repr(error)),indent=2)+'\n');raise SystemExit(repr(error))
rejected=[]
def negative(name,change):
 nn=copy.deepcopy(n);rr=copy.deepcopy(r);change(nn,rr)
 try:validate(s,nn,rr,sr,corrected)
 except (AssertionError,KeyError,TypeError,IndexError):rejected.append(name)
 else:raise AssertionError('Accepted '+name)
negative('missing-case',lambda n,r:n['cases'].pop())
negative('missing-phase',lambda n,r:n['states'].pop())
negative('duplicate-phase',lambda n,r:n['states'].append(copy.deepcopy(n['states'][0])))
negative('reordered-phase',lambda n,r:n['states'].reverse())
negative('boolean-history',lambda n,r:n['states'][0].update(undo=False))
negative('boolean-selection',lambda n,r:next(x for x in n['states']if type(x['selection'])is int).update(selection=True))
negative('missing-known-field',lambda n,r:n['states'][0]['composition'].pop('name'))
negative('wrong-title',lambda n,r:next(x for x in n['states']if x['phase']=='title-two')['composition']['leyendas'][0].update(title='Wrong'))
negative('wrong-layer-order',lambda n,r:next(x for x in n['states']if x['phase']=='placed')['composition']['leyendas'][0]['rows'].reverse())
negative('live-refresh-too-early',lambda n,r:next(x for x in n['states']if x['phase']=='layers-changed')['composition']['leyendas'][0].update(rows=['Layer 0','Hidden','New']))
negative('wrong-derived-height',lambda n,r:next(x for x in n['states']if x['phase']=='placed')['heights'].__setitem__(0,13.))
negative('false-pending',lambda n,r:next(x for x in n['states']if x['phase']=='pending').update(pending=False))
negative('first-escape-disarmed',lambda n,r:next(x for x in n['states']if x['phase']=='cancel-pending').update(mode='select'))
negative('undo-delete-restored-selection',lambda n,r:next(x for x in n['states']if x['phase']=='undo-delete').update(selection=0))
negative('missing-reader',lambda n,r:r.pop())
negative('reordered-reader',lambda n,r:r.reverse())
negative('false-ui',lambda n,r:n.update(actual_egui=False))
negative('false-atomic-refusal',lambda n,r:n['controls'].update(atomic_refusal=False))
negative('false-warm-preview',lambda n,r:n['controls'].update(warm_preview=False))
negative('unreported-source-divergence',lambda n,r:n.update(source_failed_geometry_history=False))
negative('native-inert-undo',lambda n,r:next(x for x in n['states']if x['phase']=='move-undo-source')['composition']['leyendas'][0].update(x_mm=63.,y_mm=63.))
negative('one-ulp-width',lambda n,r:next(x for x in n['states']if x['phase']=='resize-preview')['composition']['leyendas'][0].update(w_mm=math.nextafter(70.,math.inf)))
for name,change in [('opaque-sheet-loss',lambda d:d['scene']['compositions'][0].pop('vendor_legend_sheet')),('opaque-label-type-loss',lambda d:d['scene']['compositions'][0]['leyendas'][0]['vendor_legend'].update(opaque=[True,'one',None,False]))]:
 v=copy.deepcopy(raw);change(v)
 try:guard_values(v,guard)
 except (AssertionError,KeyError,TypeError,IndexError):rejected.append(name)
 else:raise AssertionError('Accepted '+name)
report=dict(result='pass',cases=4,source_exact_authoring_states=53,explicit_native_correctness_gesture_states=16,reader_pairs=4,source_gesture_exceptions=3,negative_controls=rejected,scope='Literal original authoring and saved-reader equality; native reversible gesture history is an explicit correctness difference from original read-only height failure. No physical pixel or multi-item claim.')
a.comparison.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
