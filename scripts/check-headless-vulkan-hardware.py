#!/usr/bin/env python3
"""Host orchestration of read-only, networkless CDI devbox; project code stays Rust."""
import argparse,json,subprocess,hashlib,time,uuid,os
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument("binary");p.add_argument("output");p.add_argument("--image",default="localhost/dev-base:latest");p.add_argument("--external-timeout",type=float,default=20);a=p.parse_args()
assert a.external_timeout>0,"External timeout must be positive"
binary=Path(a.binary).resolve();out=Path(a.output).resolve()
# Never widen an existing directory's permissions or overwrite prior proof.
out.mkdir(parents=True,exist_ok=False,mode=0o700)
image_id=subprocess.check_output(["podman","image","inspect",a.image,"--format","{{.Id}}"],text=True).strip()
icd=out/"nvidia-egl-icd.json";icd.write_text(json.dumps({"file_format_version":"1.0.1","ICD":{"library_path":"/usr/lib/libEGL_nvidia.so.0","api_version":"1.4.351"}}))
result={"binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"image_id":image_id,"cases":[],"scope":"Actual strict RTX5090 headless production Vulkan/resource/world proof. No WSI/display/desktop presentation, Metal, broad GPU features or performance claim."}
def retain():
 (out/"hardware-result.json").write_text(json.dumps(result,indent=2)+"\n")
def text(value):
 return value.decode(errors="replace") if isinstance(value,bytes) else value or ""
for label,extra,expected in [
 ("positive",["--device-name","5090","--output","/out/depth.png"],0),
 ("wrong-vendor",["--vendor","0x1002"],1),
 ("wrong-name",["--device-name","not-an-available-RTX"],1),
 ("unsignaled-timeout",["--device-name","5090","--timeout-control"],124)]:
 name="ingetrazo-hardware-validation-"+uuid.uuid4().hex[:12]
 # keep-id maps the caller to the image's dev UID; output remains caller-private.
 command=["podman","run","--rm","--name",name,"--read-only","--network","none","--device","nvidia.com/gpu=0","--userns","keep-id:uid=1000,gid=1000","--user","1000:1000","--security-opt","label=disable","--tmpfs","/tmp:rw","-v",str(binary)+":/probe:ro","-v",str(out)+":/out:rw","-e","VK_ICD_FILENAMES=/out/nvidia-egl-icd.json",image_id,"/probe",*extra]
 started=time.monotonic();r=None;timeout=None
 try:r=subprocess.run(command,capture_output=True,text=True,timeout=a.external_timeout)
 except subprocess.TimeoutExpired as error:timeout=error
 finally:cleanup=subprocess.run(["podman","rm","-f",name],capture_output=True,text=True,timeout=10)
 elapsed=time.monotonic()-started
 stdout=text(timeout.stdout) if timeout else r.stdout
 stderr=text(timeout.stderr) if timeout else r.stderr
 (out/(label+".log")).write_text(stdout+stderr)
 row={"case":label,"exit_code":None if timeout else r.returncode,"elapsed_seconds":elapsed,"command":command,"external_timeout":bool(timeout),"external_timeout_seconds":a.external_timeout,"cleanup_exit_code":cleanup.returncode}
 result["cases"].append(row);retain()
 if timeout:
  result["passed"]=False;result["failure"]="External timeout; partial process output and owned-container cleanup retained";retain();raise SystemExit(result["failure"])
 if r.returncode!=expected:
  result["passed"]=False;result["failure"]=f"{label}: unexpected exit {r.returncode}, expected {expected}";retain();raise SystemExit(result["failure"])
 if label=="positive":
  assert "NVIDIA GeForce RTX 5090" in stderr and "DISCRETE_GPU" in stderr
  assert "direct_mapped_bytes: 0" in stderr and "PASS17MiB actualGPUcopy/readback" in stderr
  assert "PASS native world draw2" in stderr
 elif label.startswith("wrong"):assert "software fallback refused" in stderr
 else:assert 4.5<=elapsed<15,(label,elapsed)
result["passed"]=True;retain();print(json.dumps(result,indent=2))
