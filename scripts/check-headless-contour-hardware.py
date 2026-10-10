#!/usr/bin/env python3
"""Actual strict RTX production contour draws of frozen original IGZ inputs."""
import argparse,subprocess,json,hashlib,time,uuid
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument("binary");p.add_argument("inputs");p.add_argument("output");p.add_argument("--expected-cases",type=int,default=10);a=p.parse_args()
binary=Path(a.binary).resolve();inputs=Path(a.inputs).resolve();out=Path(a.output).resolve();out.mkdir(parents=True,exist_ok=False,mode=0o700)
image_id=subprocess.check_output(["podman","image","inspect","localhost/dev-base:latest","--format","{{.Id}}"],text=True).strip()
(out/"icd.json").write_text(json.dumps({"file_format_version":"1.0.1","ICD":{"library_path":"/usr/lib/libEGL_nvidia.so.0","api_version":"1.4.351"}}))
reference=json.loads((inputs/"original-gl-results.json").read_text())["cases"];assert len(reference)==a.expected_cases
result={"source_inputs_sha256":{},"binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"image_id":image_id,"cases":[],"passed":False,"scope":"ActualRTX strict hardware offscreen GPU contours from original IGZ, GPUreadback; existing5%positive/exactzero gate,not fullpixel/WSI/Metal parity"}
def retain():(out/"result.json").write_text(json.dumps(result,indent=2)+"\n")
def check(condition,message):
 if not condition:
  result["failure"]={"case":result["cases"][-1]["case"],"error":str(message)}
  retain();raise AssertionError(message)
for row in reference:
 case=row["case"];name="ingetrazo-contour-hardware-"+uuid.uuid4().hex[:12]
 command=["podman","run","--rm","--name",name,"--read-only","--network","none","--device","nvidia.com/gpu=0","--userns","keep-id:uid=1000,gid=1000","--user","1000:1000","--security-opt","label=disable","--tmpfs","/tmp:rw","-v",str(binary)+":/probe:ro","-v",str(inputs)+":/inputs:ro","-v",str(out)+":/out:rw","-e","VK_ICD_FILENAMES=/out/icd.json",image_id,"/probe","/inputs/"+case+".igz","/out/"+case+".png"]
 started=time.monotonic()
 execution={"case":case,"command":command,"exit_code":None,"match":False}
 result["cases"].append(execution);retain()
 try:r=subprocess.run(command,capture_output=True,text=True,timeout=20)
 except subprocess.TimeoutExpired as e:
  (out/(case+".log")).write_bytes((e.stdout or b"")+(e.stderr or b""));result["failure"]={"case":case,"command":command,"elapsed":time.monotonic()-started,"error":"externaltimeout"};retain();raise
 finally:subprocess.run(["podman","rm","-f",name],capture_output=True,timeout=10)
 (out/(case+".log")).write_text(r.stdout+r.stderr)
 execution.update(exit_code=r.returncode,elapsed=time.monotonic()-started);retain()
 check(r.returncode==0,("probe exit",r.returncode,r.stdout,r.stderr))
 check("Selected headless adapter NVIDIA GeForce RTX 5090" in r.stderr and "ownerledger0" in r.stdout,"Missing strict hardware/owner cleanup proof")
 try:data=subprocess.check_output(["convert",str(out/(case+".png")),"-depth","8","rgb:-"])
 except subprocess.CalledProcessError as error:check(False,("Invalid GPU readback image",str(error)))
 check(len(data)==800*600*3,"Unexpected GPU readback extent")
 count=sum(data[i]>180 and data[i+2]>180 and data[i+1]<80 for i in range(0,len(data),3));expected=row["magenta_fill_pixels"]
 match=count==0 if expected==0 else abs(count-expected)/expected<=.05
 if case in ("hollow","hollow-seam"):
  hole_pixels=sum(data[(y*800+x)*3]>180 and data[(y*800+x)*3+2]>180 and data[(y*800+x)*3+1]<80 for y in range(280,320) for x in range(380,420))
  execution["hole_pixels"]=hole_pixels;check(hole_pixels==0,("Native filled authored hole",case,hole_pixels))
 execution.update(original=expected,native=count,match=match,elapsed=time.monotonic()-started)
 result["source_inputs_sha256"][case]=hashlib.sha256((inputs/(case+".igz")).read_bytes()).hexdigest();retain()
 check(match,("Coverage mismatch",case,expected,count))
result["passed"]=True;retain();print(f"PASS actualRTX {len(reference)} originalIGZ contour controls")
