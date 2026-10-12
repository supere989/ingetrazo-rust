#!/usr/bin/env python3
"""Bind deterministic server/cache recipe inputs to retained actual Qt observations."""
import json,sys,hashlib
from pathlib import Path
root=Path(__file__).resolve().parents[1];sys.path.insert(0,str(root/'ingetrazo'))
from PySide6.QtGui import QVector3D
from georef.datum import SceneDatum
from georef.dem import pixel_in_tile
folder=Path(sys.argv[1]);d=json.loads((folder/'original-dem.json').read_text())
for c in d['cases']:
 mode={'cache-rotated-south':'cache','async-bbox':'async','missing-corrupt':'failure','provider-switch':'switch','mapbox-decoding':'mapbox'}[c['name']]
 c['mode']=mode
 c['cache_seed']=c['tiles'] if mode in ['cache','mapbox'] else []
 c['corrupt_cache_seed']=c['tiles'][0] if mode=='failure' else None
 c['response_status']=[404 if mode=='failure'and i==len(c['tiles'])-1 else 200 for i in range(len(c['tiles']))]
 if mode=='switch':c['provider_variant'].update(name=c['name']+'-new',max_zoom=15)
 if c['name']=='async-bbox':
  doc=json.loads((folder/c['input']).read_text())['scene'];datum=SceneDatum.from_dict(doc['georef']['datum'])
  row=next(s for s in d['states']if s['case']==c['name']and s['phase']=='all-responses');hit=set()
  for sample in row['profile']['samples']:
   lat,lon,_=datum.local_to_geodetic(QVector3D(sample['x'],sample['y'],0.));x,y,_,_=pixel_in_tile(lat,lon,c['zoom']);hit.add((x,y,c['zoom']))
  bbox={(t['x'],t['y'],t['z'])for t in c['tiles']};off=bbox-hit;assert off
  c['bbox_off_path_observation']=dict(requested=[list(t)for t in sorted(bbox)],sampled=[list(t)for t in sorted(hit)],off_path_requested=[list(t)for t in sorted(off)])
(folder/'original-dem.json').write_text(json.dumps(d,indent=2)+'\n')
(folder/'contract-preparation.json').write_text(json.dumps(dict(producer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),scope='Pure recipe/cache/status declaration plus original datum/pixel coordinate mapping over retained samples; no Profile or HTTP reacquisition'),indent=2)+'\n')
