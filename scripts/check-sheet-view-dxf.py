#!/usr/bin/env python3
"""Actual frozen writer and Rust writer reopened by independent ezdxf."""
import json, os, subprocess, sys, tempfile
from pathlib import Path
import ezdxf
from acceptance_context import context
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from formats.dxf_out import save_dxf_layers

def read(path):
    doc=ezdxf.readfile(path)
    return {"layers":[(x.dxf.name,x.dxf.linetype) for x in doc.layers if x.dxf.name not in ("0","Defpoints")],"lines":[(x.dxf.layer,list(x.dxf.start),list(x.dxf.end)) for x in doc.modelspace()],"dash_pattern":[(tag.code,tag.value) for tag in doc.linetypes.get("DASHED").pattern_tags.tags]}

def main():
    directory=Path("docs/audits/sheet-view-dxf");directory.mkdir(parents=True,exist_ok=True)
    original=directory/"original.dxf";native=directory/"native.dxf"
    for path in (original,native):path.unlink(missing_ok=True)
    groups=[("  Planta General ",[[0,0,1.23456789123,-2],[1e-10,-1e10,0,0]]),("Planta General-PERFIL",[]),("Planta General-CORTE",[]),("Planta General-OCULTAS",[[2,3,4,5]],"DASHED")]
    assert save_dxf_layers(original,groups)==3
    subprocess.run(["cargo","run","--locked","-p","ingetrazo-formats","--example","sheet_dxf_fixture","--",str(native)],check=True)
    a,b=read(original),read(native);assert a==b,(a,b)
    report={"passed":True,"provenance":context(),"reader":"ezdxf "+ezdxf.__version__,"decoded":a,"scope":"Actual original/Rust format writer equality; real composer geometry and rendered export gestures remain pending. Negative nonfinite input before any bytes also asserted by native fixture."}
    (directory/"evidence.json").write_text(json.dumps(report,indent=2)+"\n")
    print("Actual original/native sheet-view DXF reader equality PASS")
if __name__=="__main__":main()
