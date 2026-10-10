import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtGui import QVector3D
from core.mesh import _key
inputs=[float(n)/32 for n in [-5,-3,-1,0,1,3,5]]+[1e15,1e15+1e9,-1e15,-1e15-1e9,3e38,0.123456789012345,1e100,float('nan'),float('inf')]
rows=[]
for x in inputs:
 try:rows.append([float(v)for v in _key(QVector3D(x,0,0))])
 except (ValueError,OverflowError):rows.append(None)
Path(sys.argv[1]).write_text(json.dumps(rows,indent=2)+'\n')
