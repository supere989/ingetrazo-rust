import subprocess,json,time
from pathlib import Path
q=lambda *a:subprocess.check_output(a,text=True).strip()
merged=q('podman','inspect','ingetrazo-publish','--format','{{.GraphDriver.Data.MergedDir}}'); image=q('podman','inspect','ingetrazo-publish','--format','{{.Image}}')
out=Path('/tmp/ingetrazo-root-annotation-gpu');out.mkdir(mode=0o700)
(out/'nvidia-egl-icd.json').write_text(json.dumps({'file_format_version':'1.0.0','ICD':{'library_path':'/usr/lib/libEGL_nvidia.so.0','api_version':'1.4.351'}}))
a=['podman','run','--rm','--name','ingetrazo-root-annotation-gpu','--read-only','--network','none','--device','nvidia.com/gpu=0','--userns','keep-id:uid=1000,gid=1000','--user','1000:1000','--security-opt','label=disable','--tmpfs','/tmp:rw']
for x,y in [('/usr/lib/libEGL.so.1','/usr/lib/libEGL.so.1'),('/usr/lib/libGLdispatch.so.0','/usr/lib/libGLdispatch.so.0'),(merged+'/tmp/ingetrazo-root-annotation-frozen-probe','/probe')]:a+=['-v',x+':'+y+':ro']
a+=['-v',str(out)+':/out:rw','-e','VK_DRIVER_FILES=/out/nvidia-egl-icd.json',image,'timeout','30s','/probe']
t=time.monotonic()
try:
 with (out/'run.log').open('w') as f:r=subprocess.run(a,stdout=f,stderr=subprocess.STDOUT,timeout=60)
 (out/'transport.json').write_text(json.dumps({'returncode':r.returncode,'elapsed_seconds':time.monotonic()-t,'image':image},indent=2)+'\n');print((out/'run.log').read_text());print('exit',r.returncode)
finally:subprocess.run(['podman','rm','--force','ingetrazo-root-annotation-gpu'],capture_output=True,timeout=10)
