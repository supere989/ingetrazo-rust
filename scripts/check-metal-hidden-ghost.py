#!/usr/bin/env python3
"""Strict frozen NVIDIA source12 versus actual Metal GPU/app route readback."""
import argparse,hashlib,json,zipfile
from pathlib import Path
NAMES=[kind+'-'+state for kind in ['face','edge','object','textured-object','nested-object','occluded-face'] for state in ['off','on']]
def original(audit):
 m=json.loads((audit/'manifest.json').read_text());raw=(audit/m['archive']).read_bytes();assert hashlib.sha256(raw).hexdigest()==m['archive_sha256']
 z=zipfile.ZipFile(audit/m['archive']);names=['original/original-alpha-results.json']+[f'original/{n}.{ext}'for n in NAMES for ext in ['png','igz']]
 for n in names:assert hashlib.sha256(z.read(n)).hexdigest()==m['files'][n],n
 r=json.loads(z.read(names[0]));assert [c['case']for c in r['cases']]==NAMES
 for c in r['cases']:assert c['ghost_samples']==4 and c['gl_info']['vendor']=='NVIDIA Corporation' and 'RTX 5090' in c['gl_info']['renderer']
 return z,names

def compare(expected,actual):assert expected==actual,'Whole RGBA difference'
def check(audit,native):
 from PIL import Image
 import io
 z,_=original(audit);r=json.loads((native/'result.json').read_text())
 assert r['backend']=='native-metal' and r['device'] and (r['width'],r['height'])==(800,600)
 assert (r['committed_bytes'],r['reserved_bytes'])==(0,0) and r['valid_texture_unhide'] is True
 assert [c['case']for c in r['cases']]==NAMES
 for c in r['cases']:
  assert c['samples']==4 and c['warm_zero_upload'] and c['pressure_refused'] and c['old_pixels_retained'] and c['flags_recalled'] and c['unhide_empty_ghosts']
  if c['case'].startswith('edge-'):assert c['unhide_route_reason'] is None and c['unhide_wire_positive'] is True
  elif c['case'].startswith('textured-object-'):assert 'missing-ghost-image.png' in c['unhide_route_reason']
  else:assert c['unhide_route_reason'] is None
  expected=Image.open(io.BytesIO(z.read('original/'+c['case']+'.png'))).convert('RGBA').tobytes()
  actual=(native/(c['case']+'.rgba')).read_bytes();assert len(actual)==800*600*4
  compare(expected,actual)
 # Same comparison rejects literal absent-pass and one-channel mutations.
 first=Image.open(io.BytesIO(z.read('original/face-on.png'))).convert('RGBA').tobytes()
 for bad in [b'',first[:10],bytes([first[0]^1])+first[1:],Image.open(io.BytesIO(z.read('original/face-off.png'))).convert('RGBA').tobytes()]:
  try:compare(first,bad)
  except AssertionError:pass
  else:raise AssertionError('Negative accepted')
 print('PASS12 whole RGBA frames, real app route/warm/refusal/recall/Unhide ledger and4 negatives')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('audit',type=Path);p.add_argument('output',type=Path);p.add_argument('--unpack',action='store_true');a=p.parse_args()
 if a.unpack:
  a.output.mkdir(parents=True,exist_ok=True);z,names=original(a.audit)
  for name in names:(a.output/Path(name).name).write_bytes(z.read(name))
 else:check(a.audit,a.output)
