import json,sys,copy
from pathlib import Path
p=Path(sys.argv[1]);a=json.loads((p/'original.json').read_text());b=json.loads((p/'native.json').read_text())
names=['positive-tie','negative-tie','neighbour-inside','below-step','above-step','diagonal-neighbour','large-finite']
def validate(x):
 assert [c['name']for c in x]==names
 assert x==a
validate(b)
old=json.loads((p/'native-baseline.json').read_text())
try:validate(old)
except AssertionError:pass
else:raise AssertionError('old weld mismatch accepted')
for variant in [b[:-1],list(reversed(b))]:
 try:validate(variant)
 except AssertionError:pass
 else:raise AssertionError('bad inventory accepted')
print(json.dumps(dict(cases=7,exact=True,old_native_rejected=True,inventory_negatives=2),indent=2))
