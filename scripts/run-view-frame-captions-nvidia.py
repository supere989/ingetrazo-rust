#!/usr/bin/env python3
"""Host transport via podman unshare; preserved private CDI, one named consumer."""
import argparse,hashlib,json,os,subprocess,time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('output');a=p.parse_args()
q=lambda *args:subprocess.check_output(args,text=True).strip()
repo='/tmp/ingetrazo-view-frame-captions';target='/tmp/ingetrazo-skp-native-target/debug';source='/tmp/ingetrazo-view-caption-proof/source'
merged=q('podman','inspect','ingetrazo-publish','--format','{{.GraphDriver.Data.MergedDir}}');image=q('podman','inspect','ingetrazo-publish','--format','{{.Image}}')
sha=q('podman','exec','ingetrazo-publish','git','-C',repo,'rev-parse','HEAD');binary_sha=q('podman','exec','ingetrazo-publish','sha256sum',target+'/examples/view_frame_caption_workflows').split()[0]
out=Path(a.output).resolve();out.mkdir(mode=0o700,exist_ok=False)
(out/'nvidia.json').write_text(json.dumps({'file_format_version':'1.0.0','ICD':{'library_path':'libGLX_nvidia.so.0','api_version':'1.3.242'}}))
name='ingetrazo-view-captions-'+str(os.getpid());command=['podman','--cdi-spec-dir','/tmp/ingetrazo-nvidia-61571/cdi','run','--rm','--name',name,'--read-only','--network','none','--device','nvidia.com/gpu=0','--userns','keep-id:uid=1000,gid=1000','--user','1000:1000','--security-opt','label=disable','--tmpfs','/tmp:rw']
mounts=[('/usr/lib/libEGL.so.1','/usr/lib/libEGL.so.1'),('/usr/lib/libGLdispatch.so.0','/usr/lib/libGLdispatch.so.0'),(merged+'/usr/lib/x86_64-linux-gnu','/usr/lib/x86_64-linux-gnu'),(merged+'/usr/bin','/usr/bin'),(merged+'/usr/share','/usr/share'),(merged+'/etc/alternatives','/etc/alternatives'),(merged+'/etc/fonts','/etc/fonts'),(merged+target,'/native'),(merged+source,'/source')]
for left,right in mounts:command+=['-v',left+':'+right+':ro']
command+=['-v',str(out)+':/out:rw','-v',str(out/'nvidia.json')+':/nvidia.json:ro','-e','VK_DRIVER_FILES=/nvidia.json','-e','HOME=/tmp/view-caption-home','-e','XDG_CONFIG_HOME=/tmp/view-caption-config',image,'/native/examples/view_frame_caption_workflows','/source','/out/native']
start=time.monotonic();code=124
try:
 with(out/'transport.log').open('w')as log:code=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,timeout=120).returncode
finally:subprocess.run(['podman','rm','--force',name],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=10)
(out/'transport.json').write_text(json.dumps({'source':sha,'binary_sha256':binary_sha,'image':image,'command':command,'returncode':code,'seconds':time.monotonic()-start,'private_cdi_sha256':hashlib.sha256(Path('/tmp/ingetrazo-nvidia-61571/cdi/nvidia.yaml').read_bytes()).hexdigest()},indent=2)+'\n')
assert code==0,(out/'transport.log').read_text();log=(out/'native/runtime.log').read_text()if(out/'native/runtime.log').exists()else(out/'transport.log').read_text()
assert 'Selected headless adapter NVIDIA GeForce RTX 5090' in log,log
assert binary_sha==q('podman','exec','ingetrazo-publish','sha256sum',target+'/examples/view_frame_caption_workflows').split()[0]
print(json.dumps({'returncode':code,'seconds':time.monotonic()-start,'binary_sha256':binary_sha}))
