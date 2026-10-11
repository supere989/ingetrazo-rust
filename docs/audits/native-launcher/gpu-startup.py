#!/usr/bin/env python3
"""Owned native NVIDIA CLI acceptance. Invoke via podman unshare python3.
Reuses the task-private matching driver bundle; changes no host installation.
"""
import argparse, hashlib, json, os, subprocess, time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument("output");a=p.parse_args()
q=lambda *args: subprocess.check_output(args,text=True).strip()
repo="/tmp/ingetrazo-next-integration"
binary="/tmp/ingetrazo-native-font-merge-target/debug/ingetrazo"
merged=q("podman","inspect","ingetrazo-publish","--format","{{.GraphDriver.Data.MergedDir}}")
image=q("podman","inspect","ingetrazo-publish","--format","{{.Image}}")
work_source=next(m["Source"] for m in json.loads(q("podman","inspect","ingetrazo-publish","--format","{{json .Mounts}}")) if m["Destination"]=="/work")
sha=q("podman","exec","ingetrazo-publish","git","-C",repo,"rev-parse","HEAD")
binary_sha=q("podman","exec","ingetrazo-publish","sha256sum",binary).split()[0]
out=Path(a.output).resolve();out.mkdir(mode=0o700,exist_ok=False)
(out/"nvidia.json").write_text(json.dumps({"file_format_version":"1.0.0","ICD":{"library_path":"libGLX_nvidia.so.0","api_version":"1.3.242"}}))
name="ingetrazo-native-launcher-"+str(os.getpid())
command=["podman","--cdi-spec-dir","/tmp/ingetrazo-nvidia-61571/cdi","run","--rm","--name",name,"--read-only","--network","none","--device","nvidia.com/gpu=0","--userns","keep-id:uid=1000,gid=1000","--user","1000:1000","--security-opt","label=disable","--tmpfs","/tmp:rw"]
mounts=[("/usr/lib/libEGL.so.1","/usr/lib/libEGL.so.1"),("/usr/lib/libGLdispatch.so.0","/usr/lib/libGLdispatch.so.0"),(merged+"/tmp/ingetrazo-parity-python","/tmp/ingetrazo-parity-python"),(merged+"/usr/lib/x86_64-linux-gnu","/usr/lib/x86_64-linux-gnu"),(merged+"/usr/bin","/usr/bin"),(merged+"/usr/share","/usr/share"),(merged+"/etc/alternatives","/etc/alternatives"),(merged+"/etc/fonts","/etc/fonts"),(merged+"/etc/ImageMagick-6","/etc/ImageMagick-6"),(merged+repo+"/scripts","/probe"),(merged+binary,"/app"),(work_source+"/ingetrazo/examples","/input")]
for left,right in mounts: command += ["-v",left+":"+right+":ro"]
command += ["-v",str(out)+":/out:rw","-v",str(out/"nvidia.json")+":/nvidia.json:ro","-e","VK_DRIVER_FILES=/nvidia.json",image,"/tmp/ingetrazo-parity-python/bin/python","/probe/check-native-vulkan-startup.py","/app","/out/host","--require-native-world","--document","/input/banca-pergola-yanque.igz"]
start=time.monotonic();code=124
try:
 with (out/"transport.log").open("w") as log: code=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,timeout=70).returncode
finally: subprocess.run(["podman","rm","--force",name],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=10)
(out/"transport.json").write_text(json.dumps({"source":sha,"binary_sha256":binary_sha,"image":image,"command":command,"returncode":code,"seconds":time.monotonic()-start},indent=2)+"\n")
assert code==0,(out/"transport.log").read_text()
runtime=(out/"host/startup.log").read_text()
assert 'Native Vulkan adapter: "NVIDIA GeForce RTX 5090"' in runtime,runtime
assert "committed_bytes=0" in runtime,runtime
assert binary_sha==q("podman","exec","ingetrazo-publish","sha256sum",binary).split()[0]
print("PASS: actual packaged CLI on RTX5090, native world, original document, two resizes, OS close, zero final ledger")
