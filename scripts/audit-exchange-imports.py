#!/usr/bin/env python3
"""Retained importer parity probes; failures are open requirements, not skipped tests."""
import argparse,json,os,subprocess,sys,tempfile
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from core.group import world_mesh
from core.mesh import Mesh
from core import solids
from formats import igz,obj,stl,dxf_in
from tests.test_obj_parts import _cube
import ezdxf

def meshes(scene):
    yield scene.mesh
    for group in scene.groups:yield world_mesh(group)
def names(scene):
    def walk(groups):
        for group in groups:yield group.name;yield from walk(group.children)
    return list(walk(scene.groups))
def positions(scene):return sorted(set(tuple(round(c,6) for c in (v.position.x(),v.position.y(),v.position.z())) for mesh in meshes(scene) for v in mesh.vertices))
def colors(scene):
    return sorted(set(tuple(round(float(x),6) for x in f.attrs["color"]) for mesh in meshes(scene) for f in mesh.faces if f.attrs and "color" in f.attrs))
def edge_segments(scene):
    return sorted(set(tuple(sorted((tuple(round(v,6) for v in (edge.a.x(),edge.a.y(),edge.a.z())),tuple(round(v,6) for v in (edge.b.x(),edge.b.y(),edge.b.z()))))) for mesh in meshes(scene) for edge in mesh.edges))
def main():
    parser=argparse.ArgumentParser();parser.add_argument("--binary",default="target/debug/ingetrazo");parser.add_argument("--output",default="docs/audits/exchange-import-evidence.json");args=parser.parse_args();app=QApplication.instance() or QApplication([]);rows=[]
    with tempfile.TemporaryDirectory() as temp:
        directory=Path(temp)
        objpath=directory/"parts.obj";(directory/"parts.mtl").write_text("newmtl red\nKd 1 0 0\n")
        objpath.write_text("mtllib parts.mtl\ng seat\nusemtl red\n"+_cube(0,1,0)+"\ng leg\nusemtl red\n"+_cube(0,0,8)+"\n")
        stlpath=directory/"cube.stl";source=Scene();obj.load_obj(source,objpath);stl.save_stl(source,stlpath)
        dxfpath=directory/"linework.dxf";doc=ezdxf.new("R2018");doc.header["$INSUNITS"]=6;doc.layers.new("SITE");doc.modelspace().add_line((0,0,0),(2,0,0),dxfattribs={"layer":"SITE"});doc.modelspace().add_lwpolyline([(0,0),(0,3),(2,3)],dxfattribs={"layer":"SITE"});doc.saveas(dxfpath)
        for path,load in [(objpath,obj.load_obj),(stlpath,stl.load_stl),(dxfpath,dxf_in.load_dxf)]:
            original=Scene();load(original,path);dest=directory/(path.stem+".igz");proc=subprocess.run([args.binary,"convert",str(path),str(dest)],capture_output=True,text=True);errors=[]
            row={"format":path.suffix,"exit_code":proc.returncode,"errors":errors}
            if proc.returncode:errors.append(proc.stderr)
            else:
                rust=Scene();igz.load_into(rust,dest)
                if positions(rust)!=positions(original):errors.append("Actual original-runtime geometry coordinates differ")
                row["original_bounds_points"]=positions(original);row["rust_bounds_points"]=positions(rust)
                if path==objpath:
                    if not {"seat","leg"}.issubset(names(rust)):errors.append("Named OBJ parts and independently selectable children lost")
                    if colors(rust)!=colors(original):errors.append("OBJ MTL face colors lost")
                    row["original_part_names"]=names(original);row["rust_part_names"]=names(rust);row["original_colors"]=colors(original);row["rust_colors"]=colors(rust)
                if path==stlpath:
                    row["original_solid_report"]=solids.solid_report(original.mesh);row["rust_solid_report"]=solids.solid_report(rust.mesh)
                    if row["original_solid_report"]!=row["rust_solid_report"]:errors.append("STL closed-solid topology/volume differs after actual Python reopen")
                if path==dxfpath:
                    if edge_segments(rust)!=edge_segments(original):errors.append("DXF LINE/LWPOLYLINE spatial segments lost or changed")
                    if "SITE" not in [layer.name for layer in rust.layers]:errors.append("DXF named layer lost")
                    row["original_segments"]=edge_segments(original);row["rust_segments"]=edge_segments(rust)
            rows.append(row)
    report={"provenance":context(),"passed":all(not r["errors"] for r in rows),"results":rows,"scope":"Named touching OBJ parts+MTL colors, STL triangles, DXF metre LINE/LWPOLYLINE/layer. Not full entity/unit/recentering/texture/import atomicity coverage."}
    Path(args.output).write_text(json.dumps(report,indent=2)+"\n");print(json.dumps({"passed":report["passed"],"results":[{"format":r["format"],"errors":r["errors"]} for r in rows]},indent=2));return 0 if report["passed"] else 1
if __name__=="__main__":raise SystemExit(main())
