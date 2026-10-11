#!/usr/bin/env python3
"""Independent finite coverage invariants; never converts strict differences to parity."""
import argparse, copy, hashlib, importlib.util, json, math
from collections import Counter
from fractions import Fraction as Q
from pathlib import Path
HERE=Path(__file__).resolve()
spec=importlib.util.spec_from_file_location('strict',HERE.with_name('check-p05-finite-kernel.py'));strict=importlib.util.module_from_spec(spec);spec.loader.exec_module(strict)
def require(condition,message):
    if not condition:raise ValueError(message)
def point(p):
    require(isinstance(p,list) and len(p)==3 and all(type(x) in (int,float) and math.isfinite(x) for x in p),'nonfinite/invalid corner')
    return tuple(Q(x) for x in p)
def cross(a,b,c):return (b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0])
def area(ring):return sum(a[0]*b[1]-b[0]*a[1] for a,b in zip(ring,ring[1:]+ring[:1]))
def newell(ring):
    n=[Q(0),Q(0),Q(0)]
    for a,b in zip(ring,ring[1:]+ring[:1]):
        n[0]+=(a[1]-b[1])*(a[2]+b[2]);n[1]+=(a[2]-b[2])*(a[0]+b[0]);n[2]+=(a[0]-b[0])*(a[1]+b[1])
    return n

def inside(p,ring):
    count=False
    for a,b in zip(ring,ring[1:]+ring[:1]):
        if cross(a,b,p)==0 and min(a[0],b[0])<=p[0]<=max(a[0],b[0]) and min(a[1],b[1])<=p[1]<=max(a[1],b[1]):return False
        if (a[1]>p[1])!=(b[1]>p[1]):
            at=a[0]+(p[1]-a[1])*(b[0]-a[0])/(b[1]-a[1])
            if p[0]<at:count=not count
    return count

def edge(a,b):return tuple(sorted((a,b)))
def proper_cross(a,b,c,d):return cross(a,b,c)*cross(a,b,d)<0 and cross(c,d,a)*cross(c,d,b)<0
def triangle_inside(p,t):
    cs=[cross(t[i],t[(i+1)%3],p) for i in range(3)]
    return all(x>0 for x in cs) or all(x<0 for x in cs)

def validate_phase(source,native,phase):
    require(native[phase]['ok'] is True,'source-success triangulation refused')
    field='mesh_face_input' if phase=='face' else 'raw_input'
    rings=source[field];outer=[point(p) for p in rings['outer']];holes=[[point(p) for p in h] for h in rings['holes'] if len(h)>=3]
    n=newell(outer)
    if source['name'] in strict.SAVE_NAMES or len(outer)<3 or n==[0,0,0]:
        require(strict.exact(source[phase],native[phase]) is None,'degenerate ordered triangles must remain exact')
        if phase=='face':require(strict.exact(source['face_normal'],native['face_normal']) is None,'degenerate normal must remain exact')
        return {'mode':'strict-degenerate','triangle_count':len(native[phase]['triangles'])}
    axis=max(range(3),key=lambda i:abs(n[i]));indices=[i for i in range(3) if i!=axis]
    def project(p):return tuple(p[i] for i in indices)
    op=[project(p) for p in outer];hp=[[project(p) for p in h] for h in holes]
    authored=set(outer+sum(holes,[]));tri3=[[point(p) for p in t] for t in native[phase]['triangles']]
    require(all(len(t)==3 for t in tri3),'triangle arity')
    require(len(tri3)==len(source[phase]['triangles']),'source triangle multiplicity')
    require(all(p in authored for t in tri3 for p in t),'non-authored corner')
    origin=outer[0]
    require(all(sum(n[i]*(p[i]-origin[i]) for i in range(3))==0 for t in tri3 for p in t),'nonplanar corner')
    projected=[[project(p) for p in t] for t in tri3];orientation=area(op)
    require(orientation!=0,'invalid projected outer')
    signed=[cross(*t) for t in projected]
    require(all(x*orientation>0 for x in signed),'zero/reversed triangle')
    target=abs(orientation)-sum(abs(area(h)) for h in hp)
    require(target>0 and sum(abs(x) for x in signed)==target,'exact outer-minus-hole area')
    boundary=Counter(edge(a,b) for r in [op]+hp for a,b in zip(r,r[1:]+r[:1]))
    # Geometric coverage only: source may bridge aligned holes with a long
    # collinear edge. Subdivide counters exactly; retain raw triangles unchanged.
    authored2={project(p) for p in authored}
    def subsegments(a,b):
        points=sorted(p for p in authored2 if cross(a,b,p)==0 and min(a[0],b[0])<=p[0]<=max(a[0],b[0]) and min(a[1],b[1])<=p[1]<=max(a[1],b[1]))
        return [edge(p,q) for p,q in zip(points,points[1:])]
    counts=Counter(k for t in projected for i in range(3) for k in subsegments(t[i],t[(i+1)%3]))
    require(all(k in counts and counts[k]==v==1 for k,v in boundary.items()),'boundary multiplicity')
    require(all(v==(1 if k in boundary else 2) for k,v in counts.items()),'internal multiplicity')
    segments=list(counts)
    for i,(a,b) in enumerate(segments):
        for c,d in segments[i+1:]:require(not proper_cross(a,b,c,d),'proper edge crossing')
    centers=[tuple(sum(t[j][i] for j in range(3))/3 for i in range(2)) for t in projected]
    for i,c in enumerate(centers):
        require(inside(c,op) and not any(inside(c,h) for h in hp),'triangle centroid outside solid domain')
        for j,t in enumerate(projected):
            if i!=j:require(not triangle_inside(c,t),'overlapping triangle interiors')
    if phase=='face':
        normal=point(native['face_normal'])
        norm2=sum(x*x for x in normal)
        require(native['face_normal']==source['face_normal'] or abs(float(norm2)-1.0)<=1e-12,'normal is neither literal source Qt value nor unit native f64')
        require(sum(normal[i]*n[i] for i in range(3))>0,'normal opposite/zero')
        require([normal[1]*n[2]-normal[2]*n[1],normal[2]*n[0]-normal[0]*n[2],normal[0]*n[1]-normal[1]*n[0]]==[0,0,0],'normal not parallel to exact Newell direction')
    return {'mode':'exact-rational-coverage','triangle_count':len(tri3),'projected_twice_area':str(target),'boundary_edges':len(boundary),'internal_edges':len(counts)-len(boundary)}

def compare(source,native):
    strict.inventory(source);strict.inventory(native)
    require(strict.exact(strict.observable(source)['save_load'],strict.observable(native)['save_load']) is None,'save/load or construction outcome differs')
    rows=[]
    for s,n in zip(source['triangulation'],native['triangulation']):
        for k in ['raw_input','mesh_face_input']:require(strict.exact(s[k],n[k]) is None,'input mutation: '+s['name'])
        results={}
        for phase in ['face','direct']:
            try:results[phase]={'accepted':True,**validate_phase(s,n,phase)}
            except (ValueError,KeyError,TypeError,IndexError) as e:results[phase]={'accepted':False,'error':str(e)}
        normal_diff=None if s['face_normal']==n['face_normal'] else {'source_qt_f32':s['face_normal'],'native':n['face_normal']}
        ordered={p:{'source':s[p],'native':n[p]} for p in ['face','direct'] if strict.exact(s[p],n[p]) is not None}
        rows.append({'name':s['name'],'phases':results,'normal_difference':normal_diff,'ordered_triangle_differences':ordered})
    return rows

def negatives(source):
    index=next(i for i,r in enumerate(source['triangulation']) if r['name']=='proper-hole')
    variants=[]
    def mutation(name,fn):
        n=copy.deepcopy(source);fn(n['triangulation'][index]);variants.append((name,n))
    mutation('missing',lambda r:r['face']['triangles'].pop())
    mutation('duplicate',lambda r:r['face']['triangles'].append(copy.deepcopy(r['face']['triangles'][0])))
    mutation('reversed',lambda r:r['face']['triangles'][0].reverse())
    mutation('crossing',lambda r:r['face']['triangles'].__setitem__(0,[[0,0,0],[8,8,0],[0,8,0]]))
    mutation('hole-covering',lambda r:r['face']['triangles'].__setitem__(0,[[2,2,0],[6,2,0],[6,6,0]]))
    mutation('input-mutation',lambda r:r['mesh_face_input']['outer'][0].__setitem__(0,1))
    mutation('normal-scaled-2x',lambda r:r.__setitem__('face_normal',[x*2 for x in r['face_normal']]))
    mutation('normal-opposite',lambda r:r.__setitem__('face_normal',[-x for x in r['face_normal']]))
    for name,n in variants:
        try:rows=compare(source,n);rejected=not all(p['accepted'] for r in rows for p in r['phases'].values())
        except ValueError:rejected=True
        require(rejected,'negative escaped: '+name)
    return [name for name,_ in variants]

def main():
    ap=argparse.ArgumentParser();ap.add_argument('native',nargs='?',type=Path);ap.add_argument('--source',type=Path);ap.add_argument('--report',type=Path)
    ap.add_argument('--initial',type=Path);ap.add_argument('--initial-sha256');args=ap.parse_args()
    root=HERE.parents[1];source=json.loads(args.source.read_text()) if args.source else strict.canonical(root)
    path=args.native/'native.json' if args.native and args.native.is_dir() else args.native
    n=json.loads(path.read_text()) if path else source
    rows=compare(source,n);accepted=all(p['accepted'] for r in rows for p in r['phases'].values())
    history_exact=None;reader_exact=None;reopened_exact=None;initial=None
    if path:
        require(args.initial is not None and args.initial_sha256 is not None,'native acceptance requires authenticated retained initial candidate')
        require(len(args.initial_sha256)==64 and hashlib.sha256(args.initial.read_bytes()).hexdigest()==args.initial_sha256,'initial candidate digest mismatch')
        old=json.loads(args.initial.read_text());old_strict=strict.compare(source,old)
        require(old_strict is not None,'initial candidate must retain strict failure')
        old_rows=compare(source,old);old_invariants=all(p['accepted'] for r in old_rows for p in r['phases'].values())
        require(not old_invariants,'initial candidate must retain changed short-ring refusal/order failure')
        initial={'sha256':args.initial_sha256,'strict_exact':False,'strict_first_mismatch':old_strict,'finite_invariants_pass':old_invariants}
        history=json.loads((path.parent/'history-native.json').read_text());expected=json.loads((root/'docs/audits/ai-degenerate-faces/original.json').read_text())
        history_exact=strict.exact(expected,history) is None;accepted &= history_exact
        reader=json.loads((path.parent/'reader-native.json').read_text());expected=[{'name':r['name'],'topology':r['load']['topology']} for r in source['save_load']]
        reader_exact=strict.exact(expected,reader) is None;accepted &= reader_exact
        reopened=json.loads((path.parent/'native-reopened.json').read_text())
        require(reopened['source_commit']==source['provenance']['source_commit'] and reopened['reader_sha256']==source['provenance']['source_files']['formats/igz.py'],'pinned reader provenance mismatch')
        require([r['name'] for r in reopened['rows']]==strict.SAVE_NAMES,'exact native reopen inventory')
        for row in reopened['rows']:
            require(row['input_sha256']==hashlib.sha256((path.parent/(row['name']+'.native.igz')).read_bytes()).hexdigest(),'native reopen input hash mismatch')
        expected=[{'name':r['name'],'ok':True,'topology':r['load']['topology']} for r in source['save_load']]
        actual=[{k:r[k] for k in ['name','ok','topology']} for r in reopened['rows']]
        reopened_exact=strict.exact(expected,actual) is None;accepted &= reopened_exact
    report={'native_compared':path is not None,'strict_exact':strict.compare(source,n) is None if path else None,'strict_first_mismatch':strict.compare(source,n) if path else None,
            'history_27_exact':history_exact,'source_reader_9_exact':reader_exact,'native_written_python_reopens_9_exact':reopened_exact,'retained_initial':initial,
            'finite_invariants_pass':accepted,'negative_controls':negatives(source),'cases':rows,'pixel_parity_claim':False,'p05_complete':False}
    if args.report:args.report.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:v for k,v in report.items() if k!='cases'}));raise SystemExit(0 if accepted else 1)
if __name__=='__main__':main()
