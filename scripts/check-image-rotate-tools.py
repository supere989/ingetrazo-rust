import json,sys,copy
from pathlib import Path
source,native,old,zero=[json.loads(Path(p).read_text())for p in sys.argv[1:]]
required=set(['selected','implicit','locked_implicit','locked_selected','hidden_selected','cancel','copy','fractional_vcb','lazy_snapshot'])
assert set(source)==set(native)==set(old)==set(zero)==required,'missing required nine families'
assert source==native,'source frame/gesture/history mismatch'
assert not source['implicit'][1]['active']==False,'implicit case must admit actual image'
assert not source['locked_implicit'][1]['active'],'locked implicit must refuse'
for key in required-{'locked_implicit'}:assert source[key]!=old[key],'old no-image failure not rejected'
assert source['selected']!=zero['selected'],'zero factory axis failure not rejected'
for field in ['frame','active','undo']:
 bad=copy.deepcopy(native);row=bad['selected'][2]
 if field=='frame':row[field][0][0]+=1
 elif field=='active':row[field]=not row[field]
 else:row[field]+=1
 assert bad!=source,'negative control accepted'
print(json.dumps({'cases':9,'states':sum(map(len,native.values())),'old_failures':8,'zero_axis_rejected':True,'mutations_rejected':3,'exact':True}))
