#!/usr/bin/env python3
"""Finite material recipes, ownership/history and actual-reader assets; no GPU claim."""
import copy,json,sys
from pathlib import Path
p=Path(sys.argv[1]);source=json.loads((p/'original-obj-mtl.json').read_text());native=json.loads((p/'native-obj-mtl.json').read_text())
expected=['color-root','color-leaf','cream-root','missing-map-root','windows-options-root','windows-options-leaf','quoted-planar-root','partial-uv-root','negative-uv-root','unknown-material-root','invalid-image-root','missing-sidecar-root','reset-library-root','multipart-root','multipart-leaf']
def strict(a,b):
 if type(a)is not type(b):
  assert type(a)in (int,float)and type(b)in(int,float),'typed mismatch'
 if isinstance(a,dict):assert a.keys()==b.keys();[strict(a[k],b[k])for k in a]
 elif isinstance(a,list):assert len(a)==len(b);[strict(x,y)for x,y in zip(a,b)]
 else:assert a==b,(a,b)
assert [c['case']for c in source['cases']]==expected
assert [(r['case'],r['phase'])for r in source['states']]==[(n,phase)for n in expected for phase in ['before','import','undo','redo','finish']]
assert [e['case']for e in source['errors']]==['bad-color','bad-name'];strict(source,native)
assets=json.loads((p/'native-asset-controls.json').read_text());assert [r['case']for r in assets]==expected
textured={'windows-options-root','windows-options-leaf','quoted-planar-root','partial-uv-root','negative-uv-root','invalid-image-root','multipart-root','multipart-leaf'}
assert all(type(r['owned_assets'])is int and r['owned_assets']==(1 if r['case']in textured else 0)for r in assets)
reopens=json.loads((p/'reopens-obj-mtl.json').read_text());assert [r['case']for r in reopens]==expected
for row in reopens:strict(row['original'],row['native'])
lifetime=json.loads((p/'resource-lifetime-controls.json').read_text());assert set(lifetime)=={'external_file_removed_before_save','both_cancel_checkpoints_retain_root_asset','root_undo_redo_restores_asset'}and all(v is True for v in lifetime.values())
mutations=[]
for name,key in [('color-root','back'),('windows-options-root','texture'),('missing-map-root','color')]:
 b=copy.deepcopy(native);row=next(r for r in b['states']if r['case']==name and r['phase']=='import');row['scene']['active']['faces'][0]['attrs'].pop(key);mutations.append(b)
for field in ['image','uvw','sw']:
 b=copy.deepcopy(native);row=next(r for r in b['states']if r['case']=='windows-options-root'and r['phase']=='import');tex=row['scene']['active']['faces'][0]['attrs']['texture']
 if field=='image':tex['path']['sha256']='0'*64
 elif field=='uvw':tex['uvw'][0]+=1
 else:tex['sw']+=1
 mutations.append(b)
for key in ['undo','redo','depth']:
 b=copy.deepcopy(native);b['states'][6][key]+=1;mutations.append(b)
b=copy.deepcopy(native);b['states'][1]['scene']['active']['faces'][0]['attrs']['opacity']=0.1;mutations.append(b)
b=copy.deepcopy(native);b['states'][0]['undo']=False;mutations.append(b)
for b in mutations:
 try:strict(source,b)
 except AssertionError:pass
 else:raise AssertionError('material/ownership mutation accepted')
print(json.dumps({'cases':15,'states':75,'malformed_sidecar_controls':2,'asset_ownership_controls':15,'actual_original_reopens':15,'negative_controls':len(mutations),'passed':True}))
