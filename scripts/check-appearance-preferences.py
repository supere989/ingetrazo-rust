#!/usr/bin/env python3
"""Literal source palette/settings and decoded asset comparison; not toolbar-layout parity."""
import copy,hashlib,json,sys
from pathlib import Path
p=Path(sys.argv[1]);source=json.loads((p/'original-appearance.json').read_text());native=json.loads((p/'native-appearance.json').read_text())
phases=['default-dark','draft-unpublished','cancel','light-40','system-light','system-dark','system-unknown','fixed-dark','legacy-large','migrated-40','invalid-fallback']
def exact(a,b):
 if type(a)is not type(b):return False
 if isinstance(a,dict):return a.keys()==b.keys()and all(exact(a[k],b[k])for k in a)
 if isinstance(a,list):return len(a)==len(b)and all(exact(x,y)for x,y in zip(a,b))
 return a==b
def check(value):
 assert [r['phase']for r in value['rows']]==phases
 assert exact(source,value),'appearance state mismatch'
check(native);negatives=0
for index,field in [(3,'window'),(4,'dark'),(6,'dark'),(8,'icon_px'),(9,'settings'),(10,'theme')]:
 bad=copy.deepcopy(native)
 if field=='window':bad['rows'][index][field][0]+=1
 elif field=='dark':bad['rows'][index][field]=not bad['rows'][index][field]
 elif field=='icon_px':bad['rows'][index][field]=24
 elif field=='settings':bad['rows'][index][field]['ui/large_toolbar_icons']='1'
 else:bad['rows'][index][field]='light'
 try:check(bad)
 except AssertionError:negatives+=1
 else:raise AssertionError('accepted '+field)
assets=json.loads((p/'source-icons.json').read_text());observed=json.loads((p/'native-icons.json').read_text());assert len(assets)==110 and assets.keys()==observed.keys()
for key,expected in assets.items():
 assert expected['size']==observed[key]['size']==[48,48]
 rgba=(p/'native-icons'/(key+'.rgba')).read_bytes();png=(p/'native-icons'/(key+'.png')).read_bytes()
 assert len(rgba)==48*48*4 and hashlib.sha256(rgba).hexdigest()==expected['rgba_sha256'],key
 assert hashlib.sha256(png).hexdigest()==expected['png_sha256'],key
 # Meaningful negative: single decoded ink byte must be detected.
 bad=bytearray(rgba);bad[0]^=1;assert hashlib.sha256(bad).hexdigest()!=expected['rgba_sha256']
controls=json.loads((p/'native-controls.json').read_text())
for key in ['unknown_after_light_falls_back_dark','unchanged_scene_history','unknown_extra_preserved','restart_invalid_value_preserved','warm_icon_cache_unchanged']:assert controls[key]is True,key
print(json.dumps({'states':11,'decoded_assets':110,'state_negatives':negatives,'asset_negatives':110,'native_controls':5,'literal_exact':True,'layout_parity_claim':False}))
