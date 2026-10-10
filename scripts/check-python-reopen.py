#!/usr/bin/env python3
"""Original runtime differential reopen: Python(source) versus Python(Rust(source)).
Use isolated Python environment with original runtime requirements installed.
"""
import json,os,subprocess,sys,tempfile
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import igz
# Checker module has hyphens, import explicitly without changing original imports.
import importlib.util
spec=importlib.util.spec_from_file_location("document_checker",Path("scripts/check-document-parity.py"));checker=importlib.util.module_from_spec(spec);spec.loader.exec_module(checker)

def main():
    app=QApplication.instance() or QApplication([])
    rows=[]
    with tempfile.TemporaryDirectory() as temp:
        os.environ["XDG_CACHE_HOME"]=temp
        for name in checker.EXPECTED:
            src=Path("ingetrazo/examples")/name
            if not src.is_file():raise SystemExit("Mandatory source fixture missing: "+str(src))
            converted=Path(temp)/("rust-"+name)
            subprocess.run([os.environ.get("INGETRAZO_BINARY","target/debug/ingetrazo"),"convert",str(src),str(converted)],check=True,capture_output=True)
            states=[]
            for i,path in enumerate((src,converted)):
                scene=Scene();igz.load_into(scene,path)
                saved=Path(temp)/(str(i)+"-"+name);igz.save_scene(scene,saved)
                states.append(checker.read(saved))
            original,original_assets=states[0];rust,rust_assets=states[1]
            errors=checker.differences(original,rust)
            asset_errors=[n for n,h in original_assets.items() if rust_assets.get(n)!=h]
            rows.append({"name":name,"errors":errors,"asset_errors":asset_errors,"source_runtime_fields":sorted(original)})
    report={"passed":all(not r["errors"] and not r["asset_errors"] for r in rows),"fixtures":rows,"scope":"Both source and Rust roundtrip reopened and serialized by pinned original Python application. Differential original runtime semantics; no interactive workflow/render proof."}
    dest=Path("docs/audits/python-reopen-evidence.json");dest.write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps({"passed":report["passed"],"fixtures":[{"name":r["name"],"differences":len(r["errors"]),"asset_differences":len(r["asset_errors"])} for r in rows]},indent=2))
    return 0 if report["passed"] else 1
if __name__=="__main__":raise SystemExit(main())
