#!/usr/bin/env python3
"""Independent OBJ/STL/DXF geometry acceptance, including holes/nested reflection."""
import argparse,json,math,os,struct,subprocess,sys,tempfile
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D,QMatrix4x4
from core.scene import Scene
from core.group import Group
from core.mesh import Mesh
from formats import igz,obj,stl
import ezdxf

def sub(a,b):return tuple(x-y for x,y in zip(a,b))
def cross(a,b):return (a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0])
def triangle_area(a,b,c):return math.sqrt(sum(x*x for x in cross(sub(b,a),sub(c,a))))/2

def decode(path):
    triangles=[]
    if path.suffix==".obj":
        vertices=[]
        for line in path.read_text().splitlines():
            parts=line.split()
            if not parts:continue
            if parts[0]=="v":vertices.append(tuple(map(float,parts[1:4])))
            elif parts[0]=="f":
                indices=[int(p.split("/")[0]) for p in parts[1:]]
                ring=[vertices[i-1 if i>0 else i] for i in indices]
                triangles += [(ring[0],ring[i],ring[i+1]) for i in range(1,len(ring)-1)]
    elif path.suffix==".stl":
        raw=path.read_bytes();count=struct.unpack_from("<I",raw,80)[0]
        assert len(raw)==84+50*count,"Binary STL triangle count/size mismatch"
        for i in range(count):
            values=struct.unpack_from("<12f",raw,84+50*i)
            normal=values[:3];triangle=(values[3:6],values[6:9],values[9:12]);computed=cross(sub(triangle[1],triangle[0]),sub(triangle[2],triangle[0]))
            assert sum(x*y for x,y in zip(normal,computed))>0,"STL normal does not match winding"
            triangles.append(triangle)
    elif path.suffix==".dxf":
        document=ezdxf.readfile(path)
        assert not document.audit().has_errors,"External DXF parser reports invalid document"
        for face in document.modelspace().query("3DFACE"):
            ring=[tuple(getattr(face.dxf,"vtx"+str(i))) for i in range(4)]
            if ring[3]==ring[2]:ring.pop()
            triangles += [(ring[0],ring[i],ring[i+1]) for i in range(1,len(ring)-1)]
    return triangles

def contains(triangle,x,y):
    a,b,c=triangle
    signs=[(q[0]-p[0])*(y-p[1])-(q[1]-p[1])*(x-p[0]) for p,q in ((a,b),(b,c),(c,a))]
    return min(signs)>=-1e-8 or max(signs)<=1e-8

def validate(triangles,require_positive_winding=True):
    errors=[];area=sum(triangle_area(*t) for t in triangles)
    if abs(area-12)>1e-6:errors.append(f"Expected holed nested surface area12, got {area}")
    if not triangles:errors.append("Required nested transformed surface absent")
    for triangle in triangles:
        if any(abs(p[2]-30)>1e-6 or not (6-1e-6<=p[0]<=10+1e-6 and 20-1e-6<=p[1]<=24+1e-6) for p in triangle):errors.append("Transform/hidden-object bounds incorrect");break
        if require_positive_winding and cross(sub(triangle[1],triangle[0]),sub(triangle[2],triangle[0]))[2]<=0:errors.append("Reflected winding did not preserve +Z orientation");break
    for i in range(16):
        for j in range(16):
            x,y=6+(i+.371)*.25,20+(j+.413)*.25
            expected=not (7<x<9 and 21<y<23)
            if any(contains(t,x,y) for t in triangles)!=expected:errors.append(f"Spatial coverage incorrect at{x},{y}")
    return {"area":area,"triangles":len(triangles),"negative_z_winding":sum(cross(sub(t[1],t[0]),sub(t[2],t[0]))[2]<0 for t in triangles),"errors":errors}

def main():
    parser=argparse.ArgumentParser();parser.add_argument("--binary",default="target/debug/ingetrazo");parser.add_argument("--output",default="docs/audits/exchange-export-evidence.json");args=parser.parse_args()
    app=QApplication.instance() or QApplication([]);results=[]
    with tempfile.TemporaryDirectory() as temp:
        directory=Path(temp);scene=Scene();mesh=Mesh();outer=[QVector3D(*p) for p in [(0,0,0),(4,0,0),(4,4,0),(0,4,0)]];inner=[QVector3D(*p) for p in [(1,1,0),(3,1,0),(3,3,0),(1,3,0)]];mesh.add_face(outer,[inner])
        child=Group(mesh,"reflected child");child.xform=QMatrix4x4();child.xform.scale(-1,1,1)
        parent=Group(name="translated parent");parent.xform=QMatrix4x4();parent.xform.translate(10,20,30);parent.children=[child]
        hidden=Group(mesh,"hidden object");hidden.hidden=True;hidden.xform=QMatrix4x4();hidden.xform.translate(100,100,100)
        scene.groups=[parent,hidden];source=directory/"source.igz";igz.save_scene(scene,source)
        original_results={}
        for extension,save in [("obj",obj.save_obj),("stl",stl.save_stl)]:
            path=directory/("original."+extension);save(scene,path);original_results[extension]=validate(decode(path),require_positive_winding=False);assert not original_results[extension]["errors"],original_results[extension]
        for extension in ("obj","stl","dxf"):
            output=directory/("rust."+extension);proc=subprocess.run([args.binary,"convert",str(source),str(output)],capture_output=True,text=True)
            result=validate(decode(output)) if proc.returncode==0 else {"errors":[proc.stderr]}
            result["format"]=extension;result["exit_code"]=proc.returncode;results.append(result)
        # An unholed rectangle must fail area and spatial coverage controls.
        wrong=[((6,20,30),(10,20,30),(10,24,30)),((6,20,30),(10,24,30),(6,24,30))]
        assert validate(wrong)["errors"]
    report={"provenance":context(),"passed":all(not r["errors"] for r in results),"negative_controls_detected":1,"original_export_reference":original_results,"results":results,"scope":"World-space holed reflected nested surface, hidden top-level object; OBJ/STL original export comparison and external DXF parsing. Rust +Z reflected winding is a correction to original -Z open-face winding; materials/textures, loose wires, broad importer entities and sheet DXF remain open."}
    Path(args.output).write_text(json.dumps(report,indent=2)+"\n");print(json.dumps({"passed":report["passed"],"results":[{"format":r["format"],"errors":len(r["errors"])} for r in results]},indent=2))
    return 0 if report["passed"] else 1
if __name__=="__main__":raise SystemExit(main())
