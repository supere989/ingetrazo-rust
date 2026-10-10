import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from core.mesh import Mesh
from PySide6.QtGui import QVector3D
results=[]
for c in json.loads((Path(__file__).resolve().parents[1]/'tools/ai-interpreter-proof/weld-cases.json').read_text()):
 m=Mesh()
 for p in c['points']:m.add_edge(QVector3D(*p),QVector3D(0,10,0))
 vs=list(m.vertices);ix={id(v):i for i,v in enumerate(vs)}
 results.append(dict(name=c['name'],vertices=[list(v.position.toTuple())for v in vs],edges=[[ix[id(e.v0)],ix[id(e.v1)]]for e in m.edges]))
print(json.dumps(results,indent=2))
