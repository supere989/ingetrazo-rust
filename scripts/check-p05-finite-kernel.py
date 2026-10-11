#!/usr/bin/env python3
"""Exact finite P05 geometry/outcome comparison; diagnostics cannot override failure."""
import argparse, copy, hashlib, json, zipfile
from pathlib import Path
SAVE_NAMES=['empty','one','two','repeated','welded-close','empty-hole','one-hole','two-hole','empty-outer-proper-hole']
TRI_NAMES=SAVE_NAMES+['collinear','concave-xy','concave-reversed','concave-yz','concave-oblique','proper-hole','equal-height-holes']
def exact(a,b,path=''):
    if isinstance(a,bool) or isinstance(b,bool):
        if type(a)!=type(b) or a!=b:return path
    elif isinstance(a,(int,float)) and isinstance(b,(int,float)):
        if a!=b:return path
    elif type(a)!=type(b):return path
    elif isinstance(a,dict):
        if a.keys()!=b.keys():return path+'/keys'
        for k in a:
            diff=exact(a[k],b[k],path+'/'+str(k))
            if diff is not None:return diff
    elif isinstance(a,list):
        if len(a)!=len(b):return path+'/length'
        for i,(x,y) in enumerate(zip(a,b)):
            diff=exact(x,y,path+'/'+str(i))
            if diff is not None:return diff
    elif a!=b:return path
    return None

def inventory(r):
    if r.get('schema')!=1:raise ValueError('schema must be 1')
    for key,names in [('save_load',SAVE_NAMES),('triangulation',TRI_NAMES)]:
        if [x.get('name') for x in r[key]]!=names:raise ValueError('exact ordered inventory required: '+key)
    for row in r['save_load']:
        for phase in ['save','load']:
            if type(row[phase].get('ok')) is not bool:raise ValueError('literal boolean outcome required')
    for row in r['triangulation']:
        for phase in ['face','direct']:
            if type(row[phase].get('ok')) is not bool:raise ValueError('literal boolean outcome required')

def observable(r):
    inventory(r)
    saves=[]
    for row in r['save_load']:
        out={k:row[k] for k in ['name','constructed_topology','reader_route','reader_estimated_corners']}
        for phase in ['save','load']:
            value=row[phase];data={'ok':value['ok']}
            if value['ok'] and phase=='load':data['topology']=value['topology']
            if not value['ok']:
                data['not_attempted']=value.get('not_attempted',False)
                if 'partial_topology' in value:data['partial_topology']=value['partial_topology']
            out[phase]=data
        saves.append(out)
    tris=[]
    for row in r['triangulation']:
        out={k:row[k] for k in ['name','raw_input','mesh_face_input','face_normal']}
        for phase in ['face','direct']:
            value=row[phase];out[phase]={'ok':value['ok']}
            if value['ok']:out[phase]['triangles']=value['triangles']
        tris.append(out)
    return {'save_load':saves,'triangulation':tris}

def compare(source,native):
    try:return exact(observable(source),observable(native))
    except (KeyError,ValueError,TypeError) as e:return 'invalid inventory/record: '+str(e)

def negative_controls(source):
    variants=[]
    n=copy.deepcopy(source);n['save_load'][0]['load']['ok']=False;variants.append(n)
    n=copy.deepcopy(source);n['save_load'][0]['load']['topology']['faces']=[];variants.append(n)
    n=copy.deepcopy(source);n['save_load'][2]['load']['topology']['edges'][0]['faces']=[0];variants.append(n)
    n=copy.deepcopy(source);n['triangulation'][10]['face']['triangles'].reverse();variants.append(n)
    n=copy.deepcopy(source);n['triangulation'][10]['face']['triangles'][0][0][0]+=2**-20;variants.append(n)
    n=copy.deepcopy(source);n['triangulation'][-1]['direct']['triangles'].pop();variants.append(n)
    n=copy.deepcopy(source);n['triangulation'][4]['direct']=copy.deepcopy(n['triangulation'][4]['face']);variants.append(n)
    n=copy.deepcopy(source);n['triangulation'][1]['name']='empty';variants.append(n)
    n=copy.deepcopy(source);n['save_load'][0]['load']['ok']=1;variants.append(n)
    assert all(compare(source,n) is not None for n in variants)
    return len(variants)

def canonical(root):
    audit=root/'docs/audits/p05-finite-kernel';manifest=json.loads((audit/'manifest.json').read_text())
    path=audit/'records.zip'
    assert hashlib.sha256(path.read_bytes()).hexdigest()==manifest['archive_sha256']
    with zipfile.ZipFile(path) as z:
        assert sorted(z.namelist())==sorted(manifest['members'])
        for name,record in manifest['members'].items():
            data=z.read(name);assert len(data)==record['bytes'] and hashlib.sha256(data).hexdigest()==record['sha256']
        return json.loads(z.read('original.json'))
def main():
    ap=argparse.ArgumentParser();ap.add_argument('native',nargs='?',type=Path);ap.add_argument('--source',type=Path);ap.add_argument('--report',type=Path);args=ap.parse_args()
    root=Path(__file__).resolve().parents[1]
    source=json.loads(args.source.read_text()) if args.source else canonical(root)
    inventory(source);count=negative_controls(source)
    mismatch=compare(source,json.loads(args.native.read_text())) if args.native else None
    report=dict(source_save_load_cases=9,source_triangulation_cases=16,negative_controls=count,native_compared=args.native is not None,
                exact=mismatch is None if args.native else None,first_mismatch=mismatch,p05_complete=False)
    if args.report:args.report.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report));raise SystemExit(1 if mismatch is not None else 0)
if __name__=='__main__':main()
