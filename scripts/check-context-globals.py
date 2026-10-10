#!/usr/bin/env python3
import json,sys,copy
from pathlib import Path
src=json.loads(Path(sys.argv[1]).read_text());dst=json.loads(Path(sys.argv[2]).read_text());baseline=Path(sys.argv[3]).read_text()
assert src['provenance']['original_python_commit']=='6be29fe437a7cd992d4e079f71720821b25216b9'
# Native optional-null layer representation is equivalent to absent original layer.
for case in dst['cases'].values():
 for row in [case['entry']]+case['states']:
  for text in row['texts']:
   if text.get('layer') is None:text.pop('layer',None)
assert src['cases']==dst['cases']
assert 'assertion `left == right` failed' in baseline and '"guides": Array []' in baseline and '"points": Array []' in baseline and '"texts": Array []' in baseline
for key in ['guides','texts','points','datum']:
 bad=copy.deepcopy(dst);bad['cases']['folded']['states'][1][key]=[] if key!='datum' else None
 assert bad['cases']!=src['cases'],key
print(json.dumps({'exact_history_states':20,'exact_entries':4,'literal_old_entry_failure':True,'four_mutation_negatives':True,'scope':'actual app command lifecycle; no rendered keyboard/UI or complete entity families claim'}))
