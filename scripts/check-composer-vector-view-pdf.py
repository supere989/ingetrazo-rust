#!/usr/bin/env python3
"""Fresh frozen-source/native controller + strict whole-page paper View gate."""
import os,sys,subprocess,hashlib,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
root=Path(sys.argv[1]);root.mkdir(parents=True,exist_ok=True)
def run(args):subprocess.run([str(v)for v in args],check=True)
# Original protocols write exclusively into this caller-selected new evidence folder.
os.environ["INGETRAZO_VIEW_PDF_OUT"]=str(root)
run([sys.executable,"scripts/probe-python-sheet-view-pdf.py"])
run([sys.executable,"scripts/probe-python-paper-boundary.py"])
run([sys.executable,"scripts/probe-qt-paper-numerals.py",root])
run(["cargo","build","--locked","-p","ingetrazo-gui","--example","sheet_view_pdf_fixture"])
run(["cargo","build","--locked","-p","ingetrazo-render","--bin","ingetrazo-render-worker"])
target=Path(os.environ.get("CARGO_TARGET_DIR","target"));binary=target/"debug/examples/sheet_view_pdf_fixture";worker=target/"debug/ingetrazo-render-worker"
run([binary,root,worker])
run(["cargo","run","--locked","-p","ingetrazo-render","--example","paper_boundary_fixture","--",root])
run(["cargo","run","--locked","-p","ingetrazo-formats","--example","qt_paper_numeral_fixture","--",root/"qt-number-observations.json"])
for dpi in (72,144,300):run([sys.executable,"scripts/check-sheet-view-vector-pdf.py",root,dpi])
(root/"fresh-gate.json").write_text(json.dumps({"passed":True,"strict_pages":78,"native_fixture_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"worker_sha256":hashlib.sha256(worker.read_bytes()).hexdigest(),"saved_view_mode":os.environ.get("INGETRAZO_VIEW_PDF_SAVED")=="1","scope":"Orthographic current/saved View line painter plus controls; no UI/default raster/text/title/full P29 claim."},indent=2))
