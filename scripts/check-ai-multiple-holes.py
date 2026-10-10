import json,copy,sys
from pathlib import Path
p=Path(sys.argv[1]) if len(sys.argv)>1 else Path(__file__).resolve().parents[1]/'docs/audits/ai-multiple-holes'
o=json.loads((p/'original.json').read_text());n=json.loads((p/'native.json').read_text())
def validate(candidate):
 if len(candidate)!=27 or len({s['name'] for s in candidate})!=27:raise AssertionError('inventory')
 if candidate!=o:raise AssertionError('document/history mismatch')
validate(n)
for mutation in range(6):
 c=copy.deepcopy(n)
 if mutation==0:c.pop()
 elif mutation==1:c[0]['faces'][0]['holes']=[]
 elif mutation==2:c[0]['vertices'].reverse()
 elif mutation==3:c[1]['redo']=0
 elif mutation==4:c[-2]['vertices']=[]
 else:c[0]['edges'][0]['faces']=[]
 try:validate(c)
 except AssertionError:pass
 else:raise AssertionError('negative accepted')
a=json.loads((p/'original-error.json').read_text());b=json.loads((p/'native-error.json').read_text())
print('27 exact states; six mutations rejected; raw exception protocols intentionally unequal')
