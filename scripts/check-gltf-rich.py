#!/usr/bin/env python3
"""Independently decode GLB accessors, PBR recipes and embedded source assets."""
import copy,hashlib,json,struct,sys
from collections import Counter
from fractions import Fraction
from pathlib import Path

def decode(path):
    raw=Path(path).read_bytes();magic,version,total=struct.unpack_from('<III',raw)
    assert (magic,version,total)==(0x46546c67,2,len(raw))
    off=12;doc=None;binary=b''
    while off<len(raw):
        size,kind=struct.unpack_from('<II',raw,off);off+=8
        assert size%4==0 and off+size<=len(raw)
        if kind==0x4e4f534a:assert doc is None;doc=json.loads(raw[off:off+size])
        elif kind==0x004e4942:binary=raw[off:off+size]
        else:raise AssertionError('unexpected chunk')
        off+=size
    assert doc['asset']['version']=='2.0'
    def accessor(i):
        acc=doc['accessors'][i];v=doc['bufferViews'][acc['bufferView']]
        code,width={5126:('f',4),5125:('I',4)}[acc['componentType']];n={'VEC3':3,'VEC2':2,'SCALAR':1}[acc['type']]
        start=v.get('byteOffset',0)+acc.get('byteOffset',0);stride=v.get('byteStride',width*n)
        assert start+max(0,acc['count']-1)*stride+n*width<=len(binary)
        return [struct.unpack_from('<'+code*n,binary,start+j*stride) for j in range(acc['count'])]
    images=[]
    for image in doc.get('images',[]):
        v=doc['bufferViews'][image['bufferView']];data=binary[v.get('byteOffset',0):v.get('byteOffset',0)+v['byteLength']]
        images.append({'name':image['name'],'mimeType':image['mimeType'],'sha256':hashlib.sha256(data).hexdigest()})
    primitives=[]
    for mesh in doc.get('meshes',[]):
        for p in mesh['primitives']:
            assert p.get('mode',4)==4
            attrs={name:accessor(i) for name,i in p['attributes'].items()}
            assert set(attrs) in ({'POSITION','NORMAL'},{'POSITION','NORMAL','TEXCOORD_0'})
            ids=[row[0] for row in accessor(p['indices'])];assert len(ids)%3==0
            triangles=[]
            for j in range(0,len(ids),3):
                vertices=[tuple(attrs[k][index] for k in sorted(attrs)) for index in ids[j:j+3]]
                # Only cyclic starting-corner order is immaterial, not winding.
                triangles.append(min(tuple(vertices[k:]+vertices[:k]) for k in range(3)))
            position=doc['accessors'][p['attributes']['POSITION']]
            primitives.append({'material':p['material'],'attributes':sorted(attrs),'triangles':sorted(triangles),'min':position['min'],'max':position['max']})
    return {'asset':doc['asset'],'materials':doc.get('materials',[]),'images':images,'textures':doc.get('textures',[]),'samplers':doc.get('samplers',[]),'extensionsUsed':doc.get('extensionsUsed',[]),'nodes':doc.get('nodes',[]),'scenes':doc['scenes'],'scene':doc['scene'],'primitives':primitives}

def planar_boundary(p):
    # A GLB's internal diagonals are not a surface property. For a solid,
    # planar primitive independently prove exact plane, oriented boundary,
    # signed area, and vertex normals with rational arithmetic on decoded f32.
    assert p['attributes']==['NORMAL','POSITION']
    triangles=p['triangles'];assert triangles
    points=[tuple(Fraction(x) for x in v[1]) for t in triangles for v in t]
    origin=points[0]
    normals={tuple(v[0]) for t in triangles for v in t};assert len(normals)==1
    cross=lambda a,b:(a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0])
    sub=lambda a,b:tuple(x-y for x,y in zip(a,b))
    plane=None
    for t in triangles:
        a,b,c=[tuple(Fraction(x) for x in v[1]) for v in t]
        n=cross(sub(b,a),sub(c,a))
        if any(n):plane=n;break
    assert plane is not None
    assert all(sum(x*y for x,y in zip(plane,sub(p,origin)))==0 for p in points)
    edges=Counter();area=[Fraction(0)]*3
    for t in triangles:
        a,b,c=[tuple(Fraction(x) for x in v[1]) for v in t]
        n=cross(sub(b,a),sub(c,a));assert sum(x*y for x,y in zip(plane,n))>=0
        area=[x+y for x,y in zip(area,n)]
        for x,y in [(a,b),(b,c),(c,a)]:
            if edges[(y,x)]:edges[(y,x)]-=1
            else:edges[(x,y)]+=1
    return (normals,tuple(area),sorted((e,n) for e,n in edges.items() if n))

def validate(a,b):
    if a==b:return
    aa=copy.deepcopy(a);bb=copy.deepcopy(b)
    for x,y in zip(aa['primitives'],bb['primitives']):
        if x['triangles']!=y['triangles']:
            assert planar_boundary(x)==planar_boundary(y),'painted surface differs'
            x['triangles']=y['triangles']=[]
    assert aa==bb,'decoded exporter semantics differ' 

def main():
    source=Path(sys.argv[1]);native=Path(sys.argv[2]);names=(source/'cases.txt').read_text().splitlines();assert len(names)==9 and len(set(names))==9
    failures=[];results={}
    for name in names:
        a=decode(source/(name+'.glb'));b=decode(native/(name+'.glb'));results[name]={'original':a,'native':b}
        try:validate(a,b)
        except AssertionError:failures.append(name)
    if len(sys.argv)>3:Path(sys.argv[3]).write_text(json.dumps(results,indent=2)+'\n')
    if failures:raise AssertionError('failed cases: '+', '.join(failures))
    a=results['textures-alpha']['original']
    mutations=[]
    for key in ['materials','images','primitives','nodes']:
        b=copy.deepcopy(a);b[key].pop();mutations.append(b)
    b=copy.deepcopy(a);b['primitives'][0]['attributes'].remove('NORMAL');mutations.append(b)
    b=copy.deepcopy(results['hole']['original']); t=list(b['primitives'][0]['triangles'][0]); v=list(t[0]); pos=list(v[1]); pos[0]+=0.125; v[1]=tuple(pos); t[0]=tuple(v); b['primitives'][0]['triangles'][0]=tuple(t); mutations.append((results['hole']['original'],b))
    b=copy.deepcopy(results['hole']['original']); b['primitives'][0]['triangles'][0]=tuple(reversed(b['primitives'][0]['triangles'][0])); mutations.append((results['hole']['original'],b))
    for b in mutations:
        try:
            if isinstance(b,tuple): validate(*b)
            else: validate(a,b)
        except AssertionError:continue
        raise AssertionError('negative accepted')
    strict=sum(v['original']==v['native'] for v in results.values())
    print(f'PASS: 9 exact decoded surfaces/recipes ({strict} identical triangle collections), 7 altered outputs rejected')
if __name__=='__main__':main()
