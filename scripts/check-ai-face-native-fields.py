"""Strict native Face metadata projection against operative source history."""
import copy,json,sys
from pathlib import Path
def validate(source,native):
 assert set(source)==set(native)=={'states','production_enabled'}
 assert source['production_enabled'] is native['production_enabled'] is False
 assert len(source['states'])==len(native['states'])==9
 assert {v['name'] for v in source['states']}=={name+'/'+phase for name in ('existing','replacement','append') for phase in ('edited','undone','redone')}
 assert source==native
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text());validate(s,n)
mutations=[]
for label,change in [('color',lambda v:v['states'][0]['faces'][0]['attrs'].__setitem__('color',[1,1,1])),('layer',lambda v:v['states'][0]['faces'][0].__setitem__('layer','Wrong')),('nested',lambda v:v['states'][0]['faces'][0]['attrs']['vendor'].__setitem__('uv',[9])),('history',lambda v:v['states'][1].__setitem__('redo',0)),('missing',lambda v:v['states'].pop()),('production',lambda v:v.__setitem__('production_enabled',True))]:
 m=copy.deepcopy(n);change(m)
 try:validate(s,m)
 except (AssertionError,KeyError,TypeError):mutations.append(label)
 else:raise AssertionError('accepted '+label)
print(json.dumps({'states':9,'rejected':mutations,'scope':'typed Face attrs/layer projection, production disabled'},indent=2))
