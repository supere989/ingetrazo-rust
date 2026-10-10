"""Execute identical actual original/native finalizer recipe and require mutation refusal."""
import os,sys,subprocess,tempfile,json
from pathlib import Path
r=Path(__file__).resolve().parents[1]
recipe=(r/'scripts/fixtures/ai-graph-finalizer.py').read_text()
binary=sys.argv[1]
bootstrap="import sys,gc;sys.path.insert(0,'/work/ingetrazo');from core.mesh import Vertex,Edge;from PySide6.QtGui import QVector3D;gc.disable();proof_collect_gc=gc.collect;exec(open(sys.argv[1]).read())"
variants={'positive':recipe,'missing-resurrection':recipe.replace('saved.append(e)','pass'),'wrong-endpoint-identity':recipe.replace('e.v0 is self.a','e.v0 is self.b'),'double-finalization':recipe.replace('assert len(events)==1','assert len(events)==2')}
results={}
with tempfile.TemporaryDirectory(prefix='ai-finalizer-check-') as tmp:
 for name,code in variants.items():
  path=Path(tmp)/(name+'.py');path.write_text(code)
  entries=[]
  for runtime,command in [('original',['/tmp/ingetrazo-parity-python/bin/python','-c',bootstrap,str(path)]),('native',[binary,str(path)])]:
   p=subprocess.run(command,cwd=r,env={**os.environ,'QT_QPA_PLATFORM':'offscreen'},capture_output=True,text=True,timeout=15)
   if name=='positive':assert p.returncode==0,(runtime,p.stderr)
   else:assert p.returncode!=0 and 'AssertionError' in p.stderr,(name,runtime,p.returncode,p.stderr)
   entries.append({'runtime':runtime,'returncode':p.returncode,'assertion_rejection':name!='positive'})
  results[name]=entries
print(json.dumps({'cases':results,'production_enabled':False,'native_callback_reentry_proven':False},indent=2))
