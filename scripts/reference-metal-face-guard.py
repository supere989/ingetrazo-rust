"""Execute frozen viewport's actual nested bucket_face without a GL process."""
import ast,inspect,json,sys,types
from array import array
sys.path.insert(0,'/work/ingetrazo')
from views.viewport import Viewport
from core.scene import Scene
from core.history import History
from core.materials import effective_attrs,back_is_default
from tests.test_pushpull_ux import _cube
source=inspect.getsourcefile(Viewport);tree=ast.parse(open(source).read())
bucket=next(n for n in ast.walk(tree) if isinstance(n,ast.FunctionDef) and n.name=='bucket_face')
guard=next(n for n in ast.walk(tree) if isinstance(n,ast.If) and ast.unparse(n.test)=='self._faces_count > 0 and mode != \'wireframe\'')
assert any(isinstance(n,ast.Attribute) and n.attr=='_fvcol_run' for n in ast.walk(guard))
rows=[]
for ordinary in [0,1,6]:
 scene=Scene();_cube(scene,History(scene))
 for i,f in enumerate(scene.mesh.faces): f.attrs.update({'color':[.3,.5,.7],'back':{'color':[.8,.2,.1],'opacity':1. if i<ordinary else .5}})
 v=types.SimpleNamespace(DEFAULT_FACE_COLOR=Viewport.DEFAULT_FACE_COLOR,_LIGHT=Viewport._LIGHT,_normal_of=lambda f:f.normal(),_tris_of=lambda f:f.triangulate())
 v._shade_factor=types.MethodType(Viewport._shade_factor,v);v._shaded_color=types.MethodType(Viewport._shaded_color,v)
 sink={'vcol':array('f'),'dback':array('f'),'tex':{}};fc=[];backs=[]
 def bucket_back(f):
  kind,payload=Viewport._bucket_back_face(v,f,f.attrs['back']);backs.append(kind);return kind in ('btcol','bttex')
 env=dict(self=v,sink=sink,ctx_paint=None,suppressed_faces=set(),bucket_back=bucket_back,_eff_attrs=effective_attrs,back_is_default=back_is_default,array=array,tcol_runs={},ttex_runs={},fcull_vcol_parts=fc)
 exec(compile(ast.Module(body=[bucket],type_ignores=[]),source,'exec'),env)
 for f in scene.mesh.faces:env['bucket_face'](f)
 n=len(sink['vcol'])//6;front=sum(len(p)//24 for p in fc)
 rows.append(dict(ordinary_faces=ordinary,consolidated_vertices=n,front_only_vertices=front,front_only_submitted=front if n else 0,back_kinds=backs,triangles=[[[p.x(),p.y(),p.z()]for p in f.triangulate()[0]]for f in scene.mesh.faces]))
print(json.dumps({'pin':'6be29fe437a7cd992d4e079f71720821b25216b9','guard':ast.unparse(guard.test),'cases':rows},indent=2))
