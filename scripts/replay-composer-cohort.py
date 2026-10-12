#!/usr/bin/env python3
"""Bounded frozen-corpus replay; strict gaps remain failed observations."""
import argparse, concurrent.futures, hashlib, json, os, subprocess, sys, time, zipfile
from pathlib import Path
PIN="6be29fe437a7cd992d4e079f71720821b25216b9"
WORKFLOWS={
 "mixed":("composer-mixed-selection","composer_mixed_workflows","original-mixed.json","native-mixed.json","reference-composer-mixed-reopens.py","check-composer-mixed.py",8,105),
 "clipboard":("composer-clipboard","composer_clipboard_workflows","original-clipboard.json","native-clipboard.json","reopen-composer-clipboard.py","check-composer-clipboard-parity.py",6,68),
 "run-previews":("composer-run-previews","composer_specialized_workflows","original-run-previews.json","native-run-previews.json","reference-composer-run-preview-reopens.py","check-composer-run-previews.py",5,106),
}
def digest(path):
 with Path(path).open("rb") as stream:return hashlib.file_digest(stream,"sha256").hexdigest()
def load(path):return json.loads(Path(path).read_text())
def authenticate(repo,kind,out):
 folder=repo/"docs/audits"/WORKFLOWS[kind][0];manifest=load(folder/"source-manifest.json");archive=folder/"source-records.zip"
 assert manifest["original_commit"]==PIN and digest(archive)==manifest["archive_sha256"]
 for binding in manifest.get("sources",[]):
  path=(repo/binding["path"]).resolve();path.relative_to(repo);assert digest(path)==binding["sha256"],binding["path"]
 historical=manifest.get("prior_checker_binding")
 if historical and historical.get("retained_path"):
  path=(repo/historical["retained_path"]).resolve();path.relative_to(repo);assert digest(path)==historical["sha256"]
 source=out/"source";source.mkdir()
 rows=manifest["members"];rows={v.get("name",v.get("path")):v for v in rows} if isinstance(rows,list) else rows
 with zipfile.ZipFile(archive) as z:
  assert len(z.namelist())==len(rows) and set(z.namelist())==set(rows)
  for name,binding in rows.items():
   data=z.read(name);assert len(data)==binding.get("size",binding.get("bytes")) and hashlib.sha256(data).hexdigest()==binding["sha256"],name
   path=(source/name).resolve();path.relative_to(source);path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data)
 return source,manifest

def run_one(repo,binaries,out,kind,revision):
 out.mkdir();result={"workflow":kind,"revision":revision,"source_pin":PIN,"fresh_source_captures":0,"commands":[],"result":"incomplete","gate_result":"fail"}
 try:
  source,manifest=authenticate(repo,kind,out);result["source_archive_sha256"]=manifest["archive_sha256"]
  if "capture_revision_binding" in manifest:result["capture_revision_binding"]=manifest["capture_revision_binding"]
  _,example,source_name,native_name,reader,checker,cases,count=WORKFLOWS[kind];native=out/"native";native.mkdir();binary=(binaries/example).resolve();assert binary.is_file();result["binary_sha256"]=digest(binary)
  env=dict(os.environ,QT_QPA_PLATFORM="offscreen",INGETRAZO_STATE_DIR=str(out/"state"),XDG_CONFIG_HOME=str(out/"config"),XDG_DATA_HOME=str(out/"data"),XDG_CACHE_HOME=str(out/"cache"))
  commands=[("native",[binary,source,native,*(["--run-previews"] if kind=="run-previews" else [])]),("reader",[sys.executable,repo/"scripts"/reader,source,native]),("checker",[sys.executable,repo/"scripts"/checker,source,native,out/"comparison.json"])]
  result["tool_sha256"]={name:digest(repo/"scripts"/name) for name in (reader,checker)}
  for label,argv in commands:
   start=time.monotonic();argv=list(map(str,argv))
   with (out/(label+".log")).open("w") as stdout,(out/(label+".stderr")).open("w") as stderr:process=subprocess.run(argv,cwd=source,env=env,stdout=stdout,stderr=stderr,timeout=240)
   result["commands"].append({"stage":label,"argv":argv,"cwd":str(source),"seconds":time.monotonic()-start,"returncode":process.returncode})
   if label!="checker":process.check_returncode()
   if label=="reader":result["native_readers_ok"]=True
  original=load(source/source_name);observed=load(native/native_name)
  assert original["source_pin"]==observed["source_pin"]==PIN and observed.get("failures",[])==[]
  assert [v["name"] for v in original["cases"]]==[v["name"] for v in observed["cases"]] and len(observed["cases"])==cases
  assert len(observed["states"])==count and [(v["case"],v["phase"]) for v in original["states"]]==[(v["case"],v["phase"]) for v in observed["states"]]
  assert len(load(native/"native-reopens.json"))==cases
  comparison=load(out/"comparison.json");strict=(comparison.get("result")=="PASS" if kind=="mixed" else comparison.get("ok") is True) and result["commands"][-1]["returncode"]==0
  result["strict_result"]="pass" if strict else "fail"
  if kind=="clipboard":
   for key in ("order_parametric_source_law","literal_all_state_history","traversal_residual_classification"):assert comparison[key]["ok"] is True,(key,comparison.get(key))
   assert len(comparison["traversal_residual_classification"]["rejected_controls"])==4
   result["gate_scope"]="Finite order-parametric clipboard transitions, all literal history counters, residual classifier and six readers; exact ordered parity remains separate"
  elif kind=="run-previews":
   assert comparison["semantic_ok"] is True
   result["gate_scope"]="Strict 106 state/path/style/pose/control and five reader semantics; primitive pixels remain independently strict"
   result["strict_pixels_ok"]=comparison.get("strict_pixels_ok") is True
  else:
   for flag in ("idle_reused","atomic_refusal","actual_pointer_ingress","foreign_ui_owned"):assert observed["controls"][flag] is True,flag
   assert [v["case"] for v in observed["control_cases"]]==[v["name"] for v in original["cases"]]
   for row in observed["control_cases"]:
    for flag in ("idle_reused","atomic_refusal","actual_pointer_ingress","foreign_ui_owned"):assert row[flag] is True
   result["gate_scope"]="Native/reader completion, exact inventory and operative runtime controls only; mixed frozen-model comparison remains strict observation, no traversal-law acceptance claim"
  result["gate_result"]="pass";result["result"]="pass" if strict else "strict-fail-observation"
 except Exception as error:result["result"]="fail";result["error"]=repr(error)
 finally:(out/"result.json").write_text(json.dumps(result,indent=2)+"\n")
 return result

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument("binaries",type=Path);parser.add_argument("output",type=Path);parser.add_argument("--workers",type=int,choices=(1,2),default=2);args=parser.parse_args()
 repo=Path(__file__).resolve().parents[1];out=args.output.resolve();assert not out.exists(),"Fresh output required";out.mkdir(parents=True)
 revision=subprocess.check_output(["git","-C",str(repo),"rev-parse","HEAD"],text=True).strip();assert subprocess.check_output(["git","-C",str(repo/"ingetrazo"),"rev-parse","HEAD"],text=True).strip()==PIN
 results={};start=time.monotonic()
 with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as pool:
  pending={pool.submit(run_one,repo,args.binaries.resolve(),out/k,k,revision):k for k in WORKFLOWS}
  for future in concurrent.futures.as_completed(pending):
   key=pending[future]
   try:results[key]=future.result()
   except Exception as error:results[key]={"result":"fail","gate_result":"fail","error":repr(error)}
   (out/"result.json").write_text(json.dumps({"revision":revision,"workflows":results,"result":"incomplete"},indent=2)+"\n")
 strict=all(v["result"]=="pass" for v in results.values());gate=all(v["gate_result"]=="pass" for v in results.values())
 result={"revision":revision,"source_pin":PIN,"fresh_source_captures":0,"workers":args.workers,"seconds":time.monotonic()-start,"workflows":results,"strict_result":"pass" if strict else "fail","gate_result":"pass" if gate else "fail","scope":"Mixed and preview strict residuals remain failed observations. Clipboard limited traversal-law acceptance requires independent classifier and literal history. No whole-workflow/platform/pixel parity claim."}
 (out/"result.json").write_text(json.dumps(result,indent=2)+"\n");print(json.dumps(result));return 0 if strict and gate else 1
if __name__=="__main__":raise SystemExit(main())
