#!/usr/bin/env python3
"""Read only nine native-written IGZs with the actual pinned original reader."""
import argparse,hashlib,json,os,subprocess,sys
from pathlib import Path
NAMES=['empty','one','two','repeated','welded-close','empty-hole','one-hole','two-hole','empty-outer-proper-hole']
def digest(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def topology(m):
    ix={id(v):i for i,v in enumerate(m.vertices)};ei={id(e):i for i,e in enumerate(m.edges)};fi={id(f):i for i,f in enumerate(m.faces)}
    return dict(vertices=[list(v.position.toTuple()) for v in m.vertices],
        adjacency=[sorted(ei[id(e)] for e in v.edges) for v in m.vertices],
        edges=[dict(vertices=[ix[id(e.v0)],ix[id(e.v1)]],faces=[fi[id(f)] for f in e.faces]) for e in m.edges],
        faces=[dict(outer=[ix[id(v)] for v in f.loop],holes=[[ix[id(v)] for v in h] for h in f.hole_loops]) for f in m.faces])
def main():
    ap=argparse.ArgumentParser();ap.add_argument('directory',type=Path);ap.add_argument('--source',type=Path,default=Path('/work/ingetrazo'));args=ap.parse_args()
    pin=subprocess.check_output(['git','-C',str(args.source),'rev-parse','HEAD'],text=True).strip()
    if pin!='6be29fe437a7cd992d4e079f71720821b25216b9':raise RuntimeError('source pin mismatch')
    os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(args.source))
    from core.scene import Scene
    from formats.igz import load_into
    rows=[]
    for name in NAMES:
        path=args.directory/(name+'.native.igz');scene=Scene();row={'name':name,'input_sha256':digest(path)}
        try:load_into(scene,path);row.update(ok=True,topology=topology(scene.mesh))
        except Exception as e:row.update(ok=False,error_type=type(e).__name__,error=str(e),partial_topology=topology(scene.mesh))
        rows.append(row)
    report={'source_commit':pin,'reader_sha256':digest(args.source/'formats/igz.py'),'helper_sha256':digest(__file__),'rows':rows}
    (args.directory/'native-reopened.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({'reopened':len(rows),'success':sum(r['ok'] for r in rows)}))
if __name__=='__main__':main()
