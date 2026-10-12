#!/usr/bin/env python3
"""Capture missing block import semantics through the original app and reader."""
import ast,json,os,sys,zipfile
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
import ezdxf
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from views.viewport import Viewport
from core.history import SnapshotImport
from core.mesh import Mesh
from core.group import Group
from core.layers import Layer
from formats import igz,dxf_in
for filename,names in [("reference-nested-import.py",["seed","vec","mesh_state","shape"]),("reference-dxf-curves.py",["curves"])]:
 tree=ast.parse(Path("scripts",filename).read_text())
 exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in names],type_ignores=[]),filename,"exec"))

def owners(scene):
 rows=[];meshes=[]
 def walk(groups,path=()):
  for index,g in enumerate(groups):
   key=path+(index,)
   rows.append({"path":list(key),"name":g.name,"component":g.component,"is_component":g.is_component(),"xform":None if g.xform is None else list(g.xform.data())})
   meshes.append((key,g.mesh));walk(g.children,key)
 walk(scene.groups)
 return {"groups":rows,"shared_mesh_pairs":[[list(a),list(b)] for i,(a,ma) in enumerate(meshes) for b,mb in meshes[i+1:] if ma is mb]}

def drawing(name):
 doc=ezdxf.new("R2010");doc.units=6;msp=doc.modelspace()
 for layer in ["Inside","First","Second","Ground"]:doc.layers.new(layer)
 def line_block(name="Part",base=(0,0,0)):
  b=doc.blocks.new(name,base_point=base)
  b.add_line((0,0,0),(2,0,1),dxfattribs={"layer":"Inside"})
  b.add_line((2,0,1),(2,3,0))
  return b
 if name in ["shared-base","shared-leaf","unit-centimetres"]:
  line_block(base=(1,2,.5))
  msp.add_blockref("Part",(4,5,2),dxfattribs={"layer":"First","rotation":30,"xscale":2,"yscale":.5,"zscale":1.5})
  msp.add_blockref("Part",(-3,4,1),dxfattribs={"layer":"Second","rotation":-45,"xscale":-1,"yscale":2})
 elif name=="case-sensitive-prototypes":
  line_block("Case")
  for key,x in [("Case",0),("case",4),("Case",8)]:msp.add_blockref(key,(x,0,0))
 elif name=="mixed-order":
  line_block()
  msp.add_blockref("Part",(3,4,0),dxfattribs={"layer":"First"})
  msp.add_line((-2,0,0),(-1,0,0),dxfattribs={"layer":"Second"})
  msp.add_line((-2,1,0),(-1,1,0))
  msp.add_blockref("Part",(7,4,0))
 elif name=="nested-lines":
  line_block("Inner",(1,-2,.5));outer=doc.blocks.new("OuterBlock",base_point=(-1,1,0))
  outer.add_line((-2,0,0),(-1,1,0),dxfattribs={"layer":"Second"})
  outer.add_blockref("Inner",(2,3,1),dxfattribs={"rotation":35,"xscale":-2,"yscale":3,"zscale":.5,"layer":"First"})
  msp.add_blockref("OuterBlock",(5,1,2),dxfattribs={"rotation":-20,"xscale":.75,"yscale":2,"layer":"Ground"})
 elif name=="nested-curves":
  inner=doc.blocks.new("InnerCurve");inner.add_circle((1,2,0),2);inner.add_arc((0,0,0),1,15,260)
  outer=doc.blocks.new("Curves");outer.add_blockref("InnerCurve",(3,1,0),dxfattribs={"rotation":25,"xscale":2,"yscale":.5})
  msp.add_blockref("Curves",(2,4,0),dxfattribs={"layer":"First"})
 elif name=="oblique-insert":
  line_block(base=(1,2,0));msp.add_blockref("Part",(3,4,5),dxfattribs={"extrusion":(1,2,3),"rotation":17,"xscale":1.5,"yscale":-2,"zscale":.5})
 elif name=="minsert-first-placement":
  line_block();msp.add_blockref("Part",(3,4,0),dxfattribs={"row_count":3,"column_count":4,"row_spacing":5,"column_spacing":7,"rotation":25})
 elif name=="missing-annotation-mixed":
  line_block();empty=doc.blocks.new("Notes");empty.add_text("annotation only")
  msp.add_blockref("Missing",(2,4,0));msp.add_blockref("Notes",(3,5,0));msp.add_blockref("Part",(7,8,0))
 elif name=="utm-recenter":
  line_block(base=(1,2,0));msp.add_blockref("Part",(700000.75,8200000.5,12.75),dxfattribs={"rotation":30})
  msp.add_blockref("Part",(700020.25,8200010.25,14.25))
  msp.add_line((699999.5,8200002.5,12.25),(700001.5,8200003.5,12.25),dxfattribs={"layer":"Ground"})
 elif name=="depth-limit":
  for depth in range(10):
   b=doc.blocks.new("Depth"+str(depth));b.add_line((depth,0,0),(depth,1,0))
   if depth<9:b.add_blockref("Depth"+str(depth+1),(0,0,1))
  msp.add_blockref("Depth0",(1,2,0))
 elif name=="block-face":
  b=line_block();b.add_3dface([(0,0,0),(2,0,0),(0,2,0)])
  msp.add_blockref("Part",(3,1,0));msp.add_blockref("Part",(6,1,0),dxfattribs={"xscale":-1})
 else:raise ValueError(name)
 return doc

app=QApplication([]);out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
assert not (out/"original-dxf-blocks.json").exists(),"Frozen capture already exists"
with zipfile.ZipFile("docs/audits/dxf-legacy-polylines/source-records.zip") as z:
 for name in ["2d-open.dxf","2d-open-input.igz"]:(out/name).write_bytes(z.read(name))
names=["shared-base","shared-leaf","unit-centimetres","case-sensitive-prototypes","mixed-order","nested-lines","nested-curves","oblique-insert","minsert-first-placement","missing-annotation-mixed","utm-recenter","depth-limit","block-face"]
rows=[];cases=[];plans=[]
for name in names:
 doc=drawing(name);filename=name+".dxf";doc.saveas(out/filename)
 win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);win.viewport=vp;s=vp.scene;seed(s)
 igz.save_scene(s,out/(name+"-input.igz"));context="leaf" if name=="shared-leaf" else "root"
 if context=="leaf":vp.begin_group_edit(s.groups[0]);vp.begin_group_edit(s.edit_group.children[0])
 def record(phase):rows.append({"case":name,"phase":phase,"depth":len(s._edit_stack),"undo":len(vp.history.undo_stack),"redo":len(vp.history.redo_stack),"scene":shape(s),"curves":curves(s),"owners":owners(s)})
 scale=.01 if name=="unit-centimetres" else 1.
 result={}
 def load(scene):result.update(dxf_in.load_dxf(scene,out/filename,scale=scale,name=name))
 record("before");vp.history.execute(SnapshotImport(load));assert vp.history.last_error is None,(name,vp.history.last_error)
 record("import");plans.append({"case":name,"report":result})
 vp.history.undo();record("undo");vp.history.redo();record("redo");vp.end_group_edit();record("finish")
 igz.save_scene(s,out/(name+"-original.igz"));vp.history.undo();record("finish-undo");vp.history.redo();record("finish-redo")
 cases.append({"name":name,"context":context,"file":filename,"unit":"cm" if scale==.01 else "m"});win.close()
(out/"original-dxf-blocks.json").write_text(json.dumps({"cases":cases,"states":rows},indent=2)+"\n")
(out/"source-loader-reports.json").write_text(json.dumps(plans,indent=2)+"\n")
print(json.dumps({"cases":len(cases),"states":len(rows),"ezdxf":ezdxf.__version__,"source_pin":"6be29fe437a7cd992d4e079f71720821b25216b9","actual_source":"SnapshotImport/dxf_in/Viewport"}))
