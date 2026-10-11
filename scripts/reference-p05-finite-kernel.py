#!/usr/bin/env python3
"""Capture missing finite P05 reader/triangulation outcomes; never rerun history."""
import argparse, hashlib, json, math, os, subprocess, sys
from pathlib import Path

def digest(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def inventory(root):
    original=json.loads((root/'tools/ai-interpreter-proof/face-degenerate-cases.json').read_text())
    concave=[[0,0,0],[4,0,0],[4,1,0],[1,1,0],[1,4,0],[0,4,0]]
    extra=[dict(name='collinear',outer=[[0,0,0],[1,0,0],[2,0,0]],holes=[])]
    variants={'concave-xy':concave,'concave-reversed':list(reversed(concave)),
              'concave-yz':[[0,x,y] for x,y,z in concave],
              'concave-oblique':[[x,y,x+y] for x,y,z in concave]}
    extra += [dict(name=n,outer=v,holes=[]) for n,v in variants.items()]
    extra += [dict(name='proper-hole',outer=[[0,0,0],[8,0,0],[8,8,0],[0,8,0]],
                   holes=[[[2,2,0],[2,6,0],[6,6,0],[6,2,0]]]),
              dict(name='equal-height-holes',outer=[[0,0,0],[12,0,0],[12,8,0],[0,8,0]],
                   holes=[[[2,2,0],[2,4,0],[4,4,0],[4,2,0]],
                          [[7,2,0],[7,4,0],[9,4,0],[9,2,0]]])]
    return original, original+extra

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('output',type=Path)
    ap.add_argument('--source',type=Path,default=Path('/work/ingetrazo'))
    ap.add_argument('--inventory-only',action='store_true'); args=ap.parse_args()
    root=Path(__file__).resolve().parents[1]; savecases,tricases=inventory(root)
    if args.inventory_only:
        print(json.dumps({'save_load':savecases,'triangulation':tricases},indent=2)); return
    pin=subprocess.check_output(['git','-C',str(args.source),'rev-parse','HEAD'],text=True).strip()
    if pin!='6be29fe437a7cd992d4e079f71720821b25216b9': raise RuntimeError('source pin mismatch')
    os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(args.source))
    from PySide6.QtGui import QVector3D as V
    from core.scene import Scene
    from core.triangulate import triangulate
    from formats.igz import save_scene,load_into
    def vector(v): return list(v.toTuple())
    def topology(m):
        ix={id(v):i for i,v in enumerate(m.vertices)};ei={id(e):i for i,e in enumerate(m.edges)};fi={id(f):i for i,f in enumerate(m.faces)}
        return dict(vertices=[vector(v.position) for v in m.vertices],
            adjacency=[sorted(ei[id(e)] for e in v.edges) for v in m.vertices],
            edges=[dict(vertices=[ix[id(e.v0)],ix[id(e.v1)]],faces=[fi[id(f)] for f in e.faces]) for e in m.edges],
            faces=[dict(outer=[ix[id(v)] for v in f.loop],holes=[[ix[id(v)] for v in h] for h in f.hole_loops]) for f in m.faces])
    def error(e): return dict(ok=False,error_type=type(e).__name__,error=str(e))
    def triangles(fn):
        try: return dict(ok=True,triangles=[[vector(p) for p in t] for t in fn()])
        except Exception as e: return error(e)
    def diagnostics(result,normal):
        if not result['ok']: return None
        areas=[]; signed=[]
        for a,b,c in result['triangles']:
            u=[b[i]-a[i] for i in range(3)];v=[c[i]-a[i] for i in range(3)]
            cross=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
            areas.append(math.sqrt(sum(x*x for x in cross))/2)
            signed.append(sum(cross[i]*normal[i] for i in range(3))/2)
        return dict(triangle_count=len(areas),zero_area_count=sum(a==0 for a in areas),area=sum(areas),signed_normal_area=sum(signed))
    args.output.mkdir(parents=True,exist_ok=True); saves=[]; tris=[]
    old=json.loads((root/'docs/audits/ai-degenerate-faces/original.json').read_text())
    for c in savecases:
        s=Scene();s.mesh.add_face([V(*p) for p in c['outer']],[[V(*p) for p in h] for h in c['holes']]);before=topology(s.mesh)
        prior=next(x for x in old if x['name']==c['name'])
        assert before=={k:prior[k] for k in before},c['name']
        path=args.output/(c['name']+'.original.igz');row=dict(name=c['name'],constructed_topology=before)
        try: save_scene(s,path);row['save']=dict(ok=True)
        except Exception as e: row['save']=error(e)
        if row['save']['ok']:
            row['archive_sha256']=digest(path);fresh=Scene()
            count=len(s.mesh.edges)+len(s.mesh.faces)*4
            row['reader_estimated_corners']=count;row['reader_route']='plain' if count<1024 else 'bulk'
            try: load_into(fresh,path);row['load']=dict(ok=True,topology=topology(fresh.mesh))
            except Exception as e: row['load']=dict(**error(e),partial_topology=topology(fresh.mesh))
        else: row['load']=dict(ok=False,not_attempted=True)
        saves.append(row)
    for c in tricases:
        s=Scene();face=s.mesh.add_face([V(*p) for p in c['outer']],[[V(*p) for p in h] for h in c['holes']])
        n=face.normal();f=triangles(face.triangulate)
        d=triangles(lambda:triangulate([V(*p) for p in c['outer']],[[V(*p) for p in h] for h in c['holes']]))
        tris.append(dict(name=c['name'],raw_input=c,mesh_face_input=dict(outer=[vector(p) for p in face.vertices],holes=[[vector(p) for p in h] for h in face.holes]),
            face_normal=vector(n),face=f,direct=d,diagnostics=dict(face=diagnostics(f,vector(n)),direct=diagnostics(d,vector(n)))))
    files=['core/mesh.py','core/triangulate.py','core/topology.py','formats/igz.py']
    report=dict(schema=1,provenance=dict(source_commit=pin,source_files={p:digest(args.source/p) for p in files},
        reference_sha256=digest(__file__),retained_history_sha256=digest(root/'docs/audits/ai-degenerate-faces/original.json'),
        input_sha256=digest(root/'tools/ai-interpreter-proof/face-degenerate-cases.json')),
        save_load=saves,triangulation=tris)
    (args.output/'original.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
    print(json.dumps(dict(save_load=len(saves),triangulation=len(tris),load_success=sum(x['load']['ok'] for x in saves))))
if __name__=='__main__': main()
