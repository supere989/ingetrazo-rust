#!/usr/bin/env python3
"""Independent retained-artifact check: exact bytes, visible frames, controls."""
import copy, hashlib, io, json, sys, zipfile
from pathlib import Path
from PIL import Image

def require(value, message):
    if not value:
        raise AssertionError(message)

def check(report, images):
    rgba, bc = report['runs']
    require(not rgba['bc_enabled'] and bc['bc_enabled'], 'feature control')
    require(rgba['device'] == bc['device'], 'adapter mismatch')
    for run in [rgba, bc]:
        require(run['ledger_final'] == 0, 'resource leak')
        require(len(run['cases']) == (10 if run['bc_enabled'] else 9), 'case coverage')
        for case in run['cases']:
            require(case['hardware_texels_exact'], 'decoded texels')
    a = next(c for c in rgba['cases'] if c['case'] == 'large-mask')
    b = next(c for c in bc['cases'] if c['case'] == 'large-mask')
    require(a['uploaded_bytes'] == 1048576 and b['uploaded_bytes'] == 262144, 'compressed upload')
    require(a['actual_image_allocation'] > b['actual_image_allocation'], 'actual GPU savings')
    for name in ['inexact', 'opaque-mipped', 'tiny']:
        require(next(c for c in bc['cases'] if c['case'] == name)['bc7_uploads'] == 0, 'fallback')
    chunk = bc['cases'][-1]
    require(chunk['uploaded_bytes'] == 8413200 and chunk['chunks'] == 2, 'block chunk coverage')
    for step in range(4):
        left, right = (images[(mode, step)] for mode in ['rgba', 'bc7'])
        require(len(left) == len(right) == 256*256*4, 'frame extent')
        require(left == right, 'full-frame mismatch')
        require(len({left[i:i+4] for i in range(0,len(left),4)}) > 16, 'blank frame')

root=Path(sys.argv[1])
manifest=json.loads((root/'manifest.json').read_text())
archive=(root/'captures.zip').read_bytes()
require(hashlib.sha256(archive).hexdigest()==manifest['archive_sha256'],'archive identity')
with zipfile.ZipFile(io.BytesIO(archive)) as z:
    require(set(z.namelist())==set(manifest['entries']),'archive inventory')
    for name,meta in manifest['entries'].items():
        data=z.read(name)
        require(len(data)==meta['bytes'] and hashlib.sha256(data).hexdigest()==meta['sha256'],'entry identity '+name)
    rows=[]
    for backend in ['nvidia-final','software-final']:
        report=json.loads(z.read(backend+'/report.json'))
        images={(mode,step):Image.open(io.BytesIO(z.read(f'{backend}/{mode}/world-{step}.png'))).convert('RGBA').tobytes() for mode in ['rgba','bc7'] for step in range(4)}
        check(report,images)
        failures=[]
        for kind in ['pixel','allocation','fallback','ledger']:
            changed=copy.deepcopy(report);frames=images.copy()
            if kind=='pixel':
                data=bytearray(frames[('bc7',1)]);data[129]^=1;frames[('bc7',1)]=bytes(data)
            if kind=='allocation':
                next(c for c in changed['runs'][1]['cases'] if c['case']=='large-mask')['actual_image_allocation']=2**40
            if kind=='fallback':
                next(c for c in changed['runs'][1]['cases'] if c['case']=='inexact')['bc7_uploads']=1
            if kind=='ledger': changed['runs'][1]['ledger_final']=1
            try: check(changed,frames)
            except AssertionError: failures.append(kind)
        require(len(failures)==4,'negative checker controls')
        rows.append({'backend':backend,'frames_exact':4,'rgba_pixels_checked':4*256*256,'negative_controls':failures})
print(json.dumps({'archive_files':len(manifest['entries']),'runs':rows},indent=2))
