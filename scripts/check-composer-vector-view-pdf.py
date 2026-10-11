#!/usr/bin/env python3
"""Fresh frozen-source/native controller + strict whole-page paper View gate."""
import argparse,os,sys,subprocess,hashlib,json,time
from pathlib import Path
from composer_vector_fixtures import build, consumers, require_consumers, provenance
from paper_source_receipt import environment, outputs
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument("root",type=Path)
parser.add_argument("--prebuilt-dir",type=Path,help="Native debug directory freshly built by the caller; never invokes Cargo")
args=parser.parse_args()
try:
    binaries=build() if args.prebuilt_dir is None else require_consumers(consumers(args.prebuilt_dir))
except ValueError as error:
    parser.error(str(error))
root=args.root;root.mkdir(parents=True,exist_ok=True)
def run(args):subprocess.run([str(v)for v in args],check=True)
# Original protocols write exclusively into this caller-selected new evidence folder.
os.environ["INGETRAZO_VIEW_PDF_OUT"]=str(root)
binding_started=time.perf_counter()
receipt={"mode":"fresh-source-instrumentation-only","historical_replay_admitted":False,"environment":environment(Path.cwd()),"probes":[]}
receipt["binding_seconds"]=time.perf_counter()-binding_started
receipt_path=root/"source-generation.json"
for name,command in [
    ("sheet-view-pdf",[sys.executable,"scripts/probe-python-sheet-view-pdf.py"]),
    ("paper-boundary",[sys.executable,"scripts/probe-python-paper-boundary.py"]),
    ("qt-numerals",[sys.executable,"scripts/probe-qt-paper-numerals.py",root]),
]:
    started=time.perf_counter();passed=False
    try:run(command);passed=True
    finally:
        receipt["probes"].append({"name":name,"seconds":time.perf_counter()-started,"passed":passed})
        receipt_path.write_text(json.dumps(receipt,indent=2)+"\n")
receipt["source_outputs"]=outputs(root)
receipt["source_seconds"]=sum(row["seconds"] for row in receipt["probes"])
receipt["scope"]="Per-recipe fresh source timing/binding; native execution and all strict page/negative checks follow unchanged. No replay or timing gain claim."
receipt_path.write_text(json.dumps(receipt,indent=2)+"\n")
binary=binaries["sheet_view_pdf_fixture"];worker=binaries["ingetrazo-render-worker"]
run([binary,root,worker])
run([binaries["paper_boundary_fixture"],root])
run([binaries["qt_paper_numeral_fixture"],root/"qt-number-observations.json"])
for dpi in (72,144,300):run([sys.executable,"scripts/check-sheet-view-vector-pdf.py",root,dpi])
(root/"fresh-gate.json").write_text(json.dumps({"passed":True,"strict_pages":78,"native_build_mode":"caller-prebuilt" if args.prebuilt_dir else "single-cargo-build","native_consumers":provenance(binaries),"native_fixture_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"worker_sha256":hashlib.sha256(worker.read_bytes()).hexdigest(),"saved_view_mode":os.environ.get("INGETRAZO_VIEW_PDF_SAVED")=="1","section_fill_mode":os.environ.get("INGETRAZO_VIEW_PDF_FILL","none"),"section_tilt":os.environ.get("INGETRAZO_VIEW_PDF_SECTION_TILT")=="1","grid_m":os.environ.get("INGETRAZO_VIEW_PDF_GRID","0"),"scope":"Orthographic current/saved View vector line, requested section fill and grid painters plus controls; no UI/default raster/text/title/full P29 claim."},indent=2))
