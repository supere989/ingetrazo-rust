#!/usr/bin/env python3
"""Host orchestration of read-only, networkless CDI devbox; project code stays Rust."""
import argparse,json,subprocess,hashlib,time,uuid
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument("binary");p.add_argument("output");p.add_argument("--image",default="localhost/dev-base:latest");a=p.parse_args()
binary=Path(a.binary).resolve();out=Path(a.output).resolve();out.mkdir(parents=True,exist_ok=True);out.chmod(0o777)
icd=out/"nvidia-egl-icd.json";icd.write_text(json.dumps({"file_format_version":"1.0.1","ICD":{"library_path":"/usr/lib/libEGL_nvidia.so.0","api_version":"1.4.351"}}))
rows=[]
for label,extra,expected in [
 ("positive",["--device-name","5090","--output","/out/depth.png"],0),
 ("wrong-vendor",["--vendor","0x1002"],1),
 ("wrong-name",["--device-name","not-an-available-RTX"],1),
 ("unsignaled-timeout",["--device-name","5090","--timeout-control"],124)]:
 name="ingetrazo-hardware-validation-"+uuid.uuid4().hex[:12]
 command=["podman","run","--rm","--name",name,"--read-only","--network","none","--device","nvidia.com/gpu=all","--security-opt","label=disable","--tmpfs","/tmp:rw","-v",str(binary)+":/probe:ro","-v",str(out)+":/out:rw","-e","VK_ICD_FILENAMES=/out/nvidia-egl-icd.json",a.image,"/probe",*extra]
 started=time.monotonic()
 try:r=subprocess.run(command,capture_output=True,text=True,timeout=20)
 finally:subprocess.run(["podman","rm","-f",name],capture_output=True)
 elapsed=time.monotonic()-started;(out/(label+".log")).write_text(r.stdout+r.stderr)
 assert r.returncode==expected,(label,r.returncode,r.stdout,r.stderr)
 if label=="positive":
  assert "NVIDIA GeForce RTX 5090" in r.stderr and "DISCRETE_GPU" in r.stderr
  assert "direct_mapped_bytes: 0" in r.stderr and "PASS17MiB actualGPUcopy/readback" in r.stderr
  assert "PASS native world draw2" in r.stderr
 elif label.startswith("wrong"):assert "software fallback refused" in r.stderr
 else:assert 4.5<=elapsed<15,(label,elapsed)
 rows.append({"case":label,"exit_code":r.returncode,"elapsed_seconds":elapsed,"command":command})
result={"binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"image_id":subprocess.check_output(["podman","image","inspect",a.image,"--format","{{.Id}}"],text=True).strip(),"cases":rows,"scope":"Actual strict RTX5090 headless production Vulkan/resource/world proof. No WSI/display/desktop presentation, Metal, broad GPU features or performance claim."}
(out/"hardware-result.json").write_text(json.dumps(result,indent=2)+"\n");print(json.dumps(result,indent=2))
