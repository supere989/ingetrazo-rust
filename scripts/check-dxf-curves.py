#!/usr/bin/env python3
"""Independent original DXF curve coverage, units boundary and refusal controls."""
import json,subprocess,tempfile,math
from pathlib import Path
from acceptance_context import context
import importlib.util
spec=importlib.util.spec_from_file_location("imports",Path(__file__).with_name("audit-exchange-imports.py"));helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import igz,dxf_in
import ezdxf
def distance(p,a,b):
    delta=[y-x for x,y in zip(a,b)];den=sum(x*x for x in delta)
    t=max(0,min(1,sum((x-y)*d for x,y,d in zip(p,a,delta))/den)) if den else 0
    return math.sqrt(sum((x-(y+t*d))**2 for x,y,d in zip(p,a,delta)))
def coverage(a,b):
    return max(min(distance(tuple(x+(y-x)*t for x,y in zip(start,end)),u,v) for u,v in b) for start,end in a for t in (0,.25,.5,.75,1))
def main():
    app=QApplication.instance() or QApplication([]);rows=[]
    with tempfile.TemporaryDirectory() as tmp:
        directory=Path(tmp)
        for kind in ("circle","arc","bulge"):
            document=ezdxf.new("R2018");document.header["$INSUNITS"]=6;space=document.modelspace()
            if kind=="circle":space.add_circle((0,0),2)
            elif kind=="arc":space.add_arc((0,0),2,10,220)
            else:space.add_lwpolyline([(0,0,1),(2,0,0)],format="xyb")
            source=directory/(kind+".dxf");document.saveas(source)
            original=Scene();dxf_in.load_dxf(original,source);out=directory/(kind+".igz")
            subprocess.run(["target/debug/ingetrazo","convert",str(source),str(out)],check=True,capture_output=True)
            rust=Scene();igz.load_into(rust,out);a,b=helpers.edge_segments(original),helpers.edge_segments(rust)
            error=max(coverage(a,b),coverage(b,a));assert error<.021,(kind,error)
            rows.append({"case":kind,"bidirectional_sampled_curve_distance":error,"tolerance_metres":.021})
        # Unit choice is an explicit original loader argument. Rust currently
        # accepts raw drawing coordinates, so millimetre scaling is retained open.
        doc=ezdxf.new("R2018");doc.header["$INSUNITS"]=4;doc.modelspace().add_line((0,0),(1000,0));source=directory/"millimetres.dxf";doc.saveas(source)
        original=Scene();dxf_in.load_dxf(original,source,scale=.001);out=directory/"millimetres.igz";subprocess.run(["target/debug/ingetrazo","convert",str(source),str(out)],check=True,capture_output=True)
        rust=Scene();igz.load_into(rust,out);units_match=helpers.positions(original)==helpers.positions(rust)

        for kind in ("block","ocs"):
            doc=ezdxf.new("R2018")
            if kind=="block":
                block=doc.blocks.new("PART");block.add_line((0,0),(1,0));doc.modelspace().add_blockref("PART",(2,3))
            else:doc.modelspace().add_circle((0,0),1,dxfattribs={"extrusion":(0,1,0)})
            source=directory/(kind+".dxf");doc.saveas(source);out=directory/(kind+".igz");out.write_bytes(b"retained-destination")
            result=subprocess.run(["target/debug/ingetrazo","convert",str(source),str(out)],capture_output=True,text=True)
            if kind=="block":
                assert result.returncode and out.read_bytes()==b"retained-destination",(kind,result.stderr)
                rows.append({"case":kind,"explicit_refusal":result.stderr.strip(),"destination_preserved":True})
            else:
                assert result.returncode==0,result.stderr
                original=Scene();dxf_in.load_dxf(original,source);rust=Scene();igz.load_into(rust,out)
                delta=max(coverage(helpers.edge_segments(original),helpers.edge_segments(rust)),coverage(helpers.edge_segments(rust),helpers.edge_segments(original)))
                assert delta<.021,(kind,delta)
                rows.append({"case":kind,"source_coverage_delta":delta,"supported_ocs":True})
    report={"provenance":context(),"passed":True,"cases":rows,"units_acceptance":units_match,"scope":"Circle/arc/bulge sampled spatial approximation to original within 2.1 cm; unsupported blocks refusal; supported OCS geometry before destination mutation. Millimetre unit conversion remains open; refusal is not entity parity."}
    Path("docs/audits/dxf-curve-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()

