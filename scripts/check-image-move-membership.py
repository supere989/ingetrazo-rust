import json,sys,copy
from pathlib import Path
source,native,old=[json.loads(Path(p).read_text())for p in sys.argv[1:]]
required={f"{m}:{e}"for m in ['reverse','delete_target','delete_other']for e in ['cancel','commit']}
assert set(source)==set(native)==set(old)==required,'missing required literal cases'
assert source==native,'identity-owned frame mismatch'
for m in ['reverse','delete_target']:
 for e in ['cancel','commit']:assert source[m+':'+e]!=old[m+':'+e],'old identity failure vanished'
for m in ['held','live']:
 bad=copy.deepcopy(native);bad['delete_target:commit'][3][m][0]=999
 assert source!=bad,'negative accepted'
print(json.dumps({'cases':6,'states':36,'old_failures':4,'negative_controls':2,'exact':True}))
