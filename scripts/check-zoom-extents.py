#!/usr/bin/env python3
"""Finite actual Zoom Extents sessions, exact state and rejecting controls."""
import argparse,copy,hashlib,json,zipfile
from pathlib import Path
NAMES=['parallel','perspective','two-point','portrait','hidden-nested','empty','orphan','figure-only','survey-only','survey-hidden','hidden-wire','hidden-face']
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
def validate(source,native):
 assert source['source_pin']==PIN
 assert [x['name']for x in source['cases']]==NAMES
 assert [x['name']for x in native['cases']]==NAMES
 for a,b in zip(source['cases'],native['cases']):
  assert b['model_history_unchanged'] is True
  for phase in ['once','twice']:assert a[phase]==b[phase],(a['name'],phase,a[phase],b[phase])
  if a['name']in ['empty','orphan','survey-hidden']:
   assert a['updates']==0
   assert a['once']=={k:v for k,v in a['initial'].items()if k in a['once']}
  else:assert a['updates']==2
 assert [x['phase'] for x in source['mode_session']['phases']]==['before','two-point','parallel','perspective']
 assert native['mode_session']['phases']==source['mode_session']['phases']
 assert native['mode_session']['model_history_unchanged'] is True
 for key in ['nonfinite_camera_refused','nonfinite_model_refused','missing_survey_refused']:assert native[key] is True
 return {'reference_sessions_exact':12,'camera_states_exact':24,'mode_transition_states_exact':4,'atomic_refusals':3,'model_history_unchanged':12}
def main():
 p=argparse.ArgumentParser();p.add_argument('--audit',type=Path,default=Path('docs/audits/zoom-extents'));p.add_argument('--native',type=Path);a=p.parse_args();manifest=json.loads((a.audit/'provenance.json').read_text());raw=(a.audit/'records.zip').read_bytes();assert hashlib.sha256(raw).hexdigest()==manifest['archive_sha256']
 with zipfile.ZipFile(a.audit/'records.zip')as z:
  assert set(z.namelist())==set(manifest['members'])
  for n,h in manifest['members'].items():assert hashlib.sha256(z.read(n)).hexdigest()==h
  source=json.loads(z.read('capture/source.json'));native=json.loads(z.read('capture/native.json'))
 if a.native:native=json.loads(a.native.read_text())
 result=validate(source,native);controls=[]
 for kind in ['empty-corpus','missing-case','duplicate-case','target','distance','mode','history','missing-refusal','resumed-two-point','hidden-wire-omitted','hidden-face-omitted']:
  bad=copy.deepcopy(native)
  if kind=='empty-corpus':bad['cases']=[]
  elif kind=='missing-case':bad['cases'].pop()
  elif kind=='duplicate-case':bad['cases'][1]=copy.deepcopy(bad['cases'][0])
  elif kind=='target':bad['cases'][1]['once']['target'][0]+=2**-20
  elif kind=='distance':bad['cases'][0]['once']['distance']+=2**-35
  elif kind=='mode':bad['cases'][2]['once']['two_point']=False
  elif kind=='history':bad['cases'][3]['model_history_unchanged']=False
  elif kind=='missing-refusal':bad.pop('missing_survey_refused')
  elif kind=='resumed-two-point':bad['mode_session']['phases'][-1]['camera']['two_point']=True
  else:bad['cases'][10 if kind=='hidden-wire-omitted' else 11]['once']=copy.deepcopy(bad['cases'][1]['once'])
  try:validate(source,bad)
  except (AssertionError,KeyError):controls.append(kind)
  else:raise AssertionError('Accepted '+kind)
 result['rejected_controls']=controls;print(json.dumps(result,indent=2))
if __name__=='__main__':main()
