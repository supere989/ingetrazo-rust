#!/usr/bin/env python3
"""Strict ordered single-picture gesture states and actual reader pairs."""
import argparse,copy,json,math,zipfile
from pathlib import Path

def equal(a,b,path='$'):
 assert type(a)is type(b),(path,'type',type(a).__name__,type(b).__name__)
 if isinstance(a,dict):
  assert a.keys()==b.keys(),(path,'keys',list(a),list(b))
  for k in a:equal(a[k],b[k],path+'.'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),(path,'length',len(a),len(b))
  for i,(x,y)in enumerate(zip(a,b)):equal(x,y,path+'.'+str(i))
 else:
  if isinstance(a,float):assert math.isfinite(a)and math.isfinite(b),(path,'nonfinite')
  assert a==b,(path,a,b)

def validate(source,native,readers,expected_readers):
 names=[c['name']for c in source['cases']];assert names==['unselected-drag','selected-drag','large-first-move','locked-only','hidden-only','ctrl-toggle','click-only','out-and-back','escape-drag','locked-overlay','hidden-overlay','higher-z','equal-z','resize-free-shift','resize-minimum','resize-minimum-snapped','drag-page-guide','resize-guide','drag-sibling','drawing-endpoint','drawing-midpoint']
 assert [c['name']for c in native['cases']]==names
 wanted=[(s['case'],s['phase'])for s in source['states']];observed=[(s['case'],s['phase'])for s in native['states']];assert len(wanted)==len(set(wanted));assert observed==wanted
 for s,n in zip(source['states'],native['states']):
  equal({k:s[k]for k in ['case','phase','composition','selection','undo','redo']},{k:n[k]for k in ['case','phase','composition','selection','undo','redo']})
 assert [r['case']for r in readers]==names and [r['case']for r in expected_readers]==names
 equal(expected_readers,readers)
 assert native['actual_egui']is True
 assert native['unknown_metadata']is True

p=argparse.ArgumentParser();p.add_argument('source',type=Path);p.add_argument('native',type=Path);p.add_argument('comparison',type=Path);a=p.parse_args()
s=json.loads((a.source/'original-gestures.json').read_text());guards=json.loads((a.source/'opaque-guards.json').read_text());first=json.loads((a.source/'first-original-gestures-guide-fixture.json').read_text());corrected={'resize-minimum-snapped','drag-page-guide','resize-guide'};assert [c['name']for c in first['cases']]==[c['name']for c in s['cases']];assert [(x['case'],x['phase'])for x in first['states']]==[(x['case'],x['phase'])for x in s['states']];assert all(x==y for x,y in zip(first['states'],s['states'])if x['case']not in corrected);assert all(any(x!=y for x,y in zip(first['states'],s['states'])if x['case']==name)for name in corrected);n=json.loads((a.native/'native-gestures.json').read_text());r=json.loads((a.native/'native-reopens.json').read_text());sr=json.loads((a.source/'original-reopens.json').read_text());a.comparison.parent.mkdir(parents=True,exist_ok=True)
def validate_raw_guards(doc):
 for dotted,expected in guards['paths'].items():
  actual=doc
  for key in dotted.split('.'):actual=actual[int(key)]if isinstance(actual,list)else actual[key]
  equal(expected,actual,'raw.'+dotted)
def saved_raw_guards():
 case=next(c for c in n['cases']if c['name']==guards['case']);path=(a.native/case['saved']).resolve();assert path.is_relative_to(a.native.resolve());raw=path.read_bytes();doc=json.loads(raw)if raw[:2]!=b'PK'else json.loads(zipfile.ZipFile(path).read('document.json'))
 validate_raw_guards(doc);return doc
try:validate(s,n,r,sr);raw_doc=saved_raw_guards()
except (AssertionError,KeyError,TypeError,IndexError)as error:
 a.comparison.write_text(json.dumps(dict(pass_=False,stage='baseline',error=repr(error),cases=len(s['cases']),states=len(s['states'])),indent=2)+'\n');raise SystemExit('Picture gesture mismatch: '+repr(error))
rejected=[]
def negative(name,mutate):
 nn=copy.deepcopy(n);rr=copy.deepcopy(r);mutate(nn,rr)
 try:validate(s,nn,rr,sr)
 except (AssertionError,KeyError,TypeError,IndexError):rejected.append(name)
 else:raise AssertionError('Checker accepted '+name)
negative('missing-state',lambda n,r:n['states'].pop())
negative('extra-state',lambda n,r:n['states'].append(copy.deepcopy(n['states'][0])))
negative('reordered-state',lambda n,r:n['states'].reverse())
negative('bool-history',lambda n,r:n['states'][0].update(undo=False))
negative('false-selection',lambda n,r:next(x for x in n['states']if type(x['selection'])is int).update(selection=True))
negative('changed-width',lambda n,r:n['states'][0]['composition']['images'][0].update(w_mm=math.nextafter(n['states'][0]['composition']['images'][0]['w_mm'],math.inf)))
negative('changed-position',lambda n,r:n['states'][0]['composition']['images'][0].update(x_mm=41.))
negative('changed-opacity',lambda n,r:n['states'][0]['composition']['images'][0].update(opacity=.5))
negative('lost-path',lambda n,r:n['states'][0]['composition']['images'][0].pop('path'))
negative('bool-hidden',lambda n,r:n['states'][0]['composition']['images'][0].update(hidden=0))
negative('missing-reader',lambda n,r:r.pop())
negative('reordered-reader',lambda n,r:r.reverse())
negative('reader-width',lambda n,r:r[0]['composition']['images'][0].update(w_mm=999.))
negative('wrong-case',lambda n,r:n['cases'][0].update(name='substitute'))
negative('synthetic-input',lambda n,r:n.update(actual_egui=False))
negative('raw-loss',lambda n,r:n.update(unknown_metadata=False))
for name,mutate in [('raw-sheet-key-loss',lambda d:d['scene']['compositions'][0].pop('vendor_gesture_sheet')),('raw-picture-type-loss',lambda d:d['scene']['compositions'][0]['images'][0]['vendor_gesture_picture'].update(opaque=[True,'one',None,False]))]:
 dd=copy.deepcopy(raw_doc);mutate(dd)
 try:validate_raw_guards(dd)
 except (AssertionError,KeyError,TypeError,IndexError):rejected.append(name)
 else:raise AssertionError('Checker accepted '+name)
report=dict(pass_=True,cases=len(s['cases']),states=len(s['states']),original_reader_pairs=len(r),rejected_controls=len(rejected),negative_names=rejected,scope='Strict known composition, owner selection, gesture history and actual original reader; source clocks diagnostic. Single picture only, no rendered pixels.')
a.comparison.parent.mkdir(parents=True,exist_ok=True);a.comparison.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
