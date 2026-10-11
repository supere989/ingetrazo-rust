#!/usr/bin/env python3
"""Run compiled integration consumers in an owned, read-only NVIDIA CDI devbox.
Invoke the exported transport using podman unshare python3; no host GUI execution.
"""
import subprocess,json,time,hashlib,os
from pathlib import Path
query=lambda *a:subprocess.check_output(a,text=True).strip()
root='/tmp/ingetrazo-next-integration'
work='/tmp/ingetrazo-shared-cache-camera-integration'
binary='/tmp/ingetrazo-native-font-merge-target/debug/examples/native_vulkan_face_sweep_application_probe'
build=json.loads(query('podman','exec','ingetrazo-publish','cat','/tmp/ingetrazo-shared-cache-camera-final/build-provenance.json'));sha=build['source']
assert not query('podman','exec','ingetrazo-publish','git','-C',root,'diff',sha,'--','crates','Cargo.toml','Cargo.lock'),'Runtime tree differs from tested revision'
binarysha=query('podman','exec','ingetrazo-publish','sha256sum',binary).split()[0]
assert binarysha==build['binary_sha256']
merged=query('podman','inspect','ingetrazo-publish','--format','{{.GraphDriver.Data.MergedDir}}')
image=query('podman','inspect','ingetrazo-publish','--format','{{.Image}}')
out=Path('/tmp/ingetrazo-shared-cache-camera-final-gpu');out.mkdir(mode=0o700)
(out/'nvidia.json').write_text(json.dumps({'file_format_version':'1.0.0','ICD':{'library_path':'libGLX_nvidia.so.0','api_version':'1.3.242'}}))
checks=[('ordinary-workflow','ordinary-prism',False,False,False),('textured-workflow','front-texture-back-override',False,False,False),('resize-refusal','ordinary-prism',False,False,True)]
checks += [(('portable-' if portable else 'native-')+name,name,True,portable,False) for portable in [False,True] for name in ['reversed-door','lone-sheet','front-recess','back-recess','front-texture-back-override','ordinary-prism']]
rows=[]
for label,case,source_frames,portable,resize_refusal in checks:
 d=out/label;d.mkdir();name='ingetrazo-integrated-gpu-'+str(os.getpid())+'-'+label
 podman=['podman']
 if os.environ.get('INGETRAZO_TEST_CDI_DIR'):podman+=['--cdi-spec-dir',os.environ['INGETRAZO_TEST_CDI_DIR']]
 args=podman+['run','--rm','--name',name,'--read-only','--network','none','--device','nvidia.com/gpu=0','--userns','keep-id:uid=1000,gid=1000','--user','1000:1000','--security-opt','label=disable','--tmpfs','/tmp:rw']
 for left,right in [('/usr/lib/libEGL.so.1','/usr/lib/libEGL.so.1'),('/usr/lib/libGLdispatch.so.0','/usr/lib/libGLdispatch.so.0'),(merged+'/tmp/ingetrazo-parity-python','/tmp/ingetrazo-parity-python'),(merged+'/usr/lib/x86_64-linux-gnu','/usr/lib/x86_64-linux-gnu'),(merged+'/usr/bin','/usr/bin'),(merged+'/usr/share','/usr/share'),(merged+'/etc/alternatives','/etc/alternatives'),(merged+'/etc/fonts','/etc/fonts'),(merged+'/etc/ImageMagick-6','/etc/ImageMagick-6'),(merged+root+'/scripts','/probe'),(merged+binary,'/app'),(merged+work+'/sweep-input','/input')]:args+=['-v',left+':'+right+':ro']
 if portable:args+=['-e','INGETRAZO_DIAGNOSTIC_PORTABLE_SOURCE_DEPTH=1']
 args+=['-v',str(d)+':/out:rw','-v',str(out/'nvidia.json')+':/nvidia.json:ro','-e','VK_DRIVER_FILES=/nvidia.json','-e','XDG_CACHE_HOME=/tmp/cache',image,'/tmp/ingetrazo-parity-python/bin/python','/probe/check-native-face-sweep-application.py','/app','/out','--source',sha,'--source-dir','/input','--case',case]
 if source_frames:args+=['--source-frames']
 if resize_refusal:args+=['--resize-refusal']
 start=time.monotonic();rc=124
 try:
  with(d/'transport.log').open('w') as f:rc=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT,timeout=70).returncode
 finally:subprocess.run(['podman','rm','--force',name],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=10)
 meta={'source':sha,'binary_sha256':binarysha,'image':image,'command':args,'returncode':rc,'seconds':time.monotonic()-start}
 (d/'transport.json').write_text(json.dumps(meta,indent=2)+'\n')
 assert rc==0,(label,rc)
 log=(d/'runtime.log').read_text();assert 'Native Vulkan adapter: "NVIDIA GeForce RTX 5090"' in log and 'committed_bytes=0' in log
 result=json.loads((d/'result.json').read_text());assert result['passed'] and result['source_sha']==sha and result['binary_sha256']==binarysha
 if source_frames:
  comparison=json.loads((d/'source-comparison.json').read_text());assert comparison['passed'] and len(comparison['comparisons'])==4,comparison
 rows.append({'name':label,'actions':len(result['rows']),'source_frames':4 if source_frames else 0,'portable':portable,'resize_refusal':resize_refusal,'seconds':meta['seconds'],'passed':True})
 (out/'summary.json').write_text(json.dumps({'source':sha,'binary_sha256':binarysha,'rows':rows},indent=2)+'\n')
 print(label,'PASS',flush=True)
assert binarysha==query('podman','exec','ingetrazo-publish','sha256sum',binary).split()[0]
