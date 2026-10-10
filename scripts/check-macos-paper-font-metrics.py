#!/usr/bin/env python3
"""Exact native/source raw glyph metrics, separate from physical and pixel gates."""
import json,sys
from pathlib import Path
NAMES=['helvetica','helvetica-bold','helvetica-italic','times-bold-italic','cjk','arabic']
def check(source,native):
 assert source['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9' and source['qt']=='6.12.0'
 assert [row['name']for row in source['cases']]==NAMES and len(native)==6
 rows=[]
 for src,got in zip(source['cases'],native):
  errors=[];diagnostics={}
  for key in ['family','bold','italic','text']:
   if src[key]!=got.get(key):errors.append('request mismatch: '+key)
  if 'error'in got:errors.append('native refusal: '+str(got['error']))
  else:
   runs=[run for call in src['observations']for run in call['supplementary_qtextlayout_runs']]
   if len(runs)!=1:errors.append('one actual source raw run required for this corpus')
   else:
    run=runs[0];raw=run.get('raw_metrics')
    if not raw:errors.append('source raw metric observations missing')
    else:
     values=[raw[key]*64 for key in ['ascent','descent','leading','cap_height']]
     diagnostics['source_metrics_26_6']=values;diagnostics['native_metrics_26_6']=got.get('metrics')
     if values!=got.get('metrics'):errors.append('exact source raw font metrics differ')
    ids=[g['id']for g in got.get('glyphs',[])]
    diagnostics['source_ids']=run['glyphs'];diagnostics['native_ids']=ids
    if run['glyphs']!=ids:errors.append('source run glyph order/identity differs')
    diagnostics['source_raw_glyph_metrics']=run.get('glyph_metrics',[])
    diagnostics['source_positions']=run['positions'];diagnostics['native_shape']=got.get('glyphs',[])
    # Do not equate unkerned raw advances with shaped advances or supplementary
    # layout positions with operative painter positions. Retain both for diagnosis.
  rows.append({'case':src['name'],'passed':not errors,'errors':errors,'diagnostics':diagnostics})
 return {'passed':all(row['passed']for row in rows),'cases':rows,'scope':'Exact raw font metrics/glyph identities only; shaping positions, synthesis and actual Canvas/PDF pixels require separate gates'}
if __name__=='__main__':
 result=check(json.loads(Path(sys.argv[1]).read_text()),json.loads(Path(sys.argv[2]).read_text()));print(json.dumps(result,indent=2,ensure_ascii=False));sys.exit(0 if result['passed']else 1)
