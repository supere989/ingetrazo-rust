import subprocess,os,json,time,sys
from pathlib import Path
q=lambda *a:subprocess.check_output(a,text=True).strip()
merged=q('podman','inspect','ingetrazo-publish','--format','{{.GraphDriver.Data.MergedDir}}');image=q('podman','inspect','ingetrazo-publish','--format','{{.Image}}');source=q('podman','inspect','ingetrazo-publish','--format','{{range .Mounts}}{{if eq .Destination "/work"}}{{.Source}}{{end}}{{end}}')
out=Path(sys.argv[1]);out.mkdir(mode=0o700);work='/tmp/ingetrazo-next-integration';name='ingetrazo-depth-oracle-root'
a=['podman','run','--rm','--name',name,'--read-only','--network','none','--device','nvidia.com/gpu=0','--userns','keep-id:uid=1000,gid=1000','--user','1000:1000','--security-opt','label=disable','--tmpfs','/tmp:rw']
for x,y in [('/usr/lib/libEGL.so.1','/usr/lib/libEGL.so.1'),('/usr/lib/libGLdispatch.so.0','/usr/lib/libGLdispatch.so.0'),(merged+'/tmp/ingetrazo-parity-python','/tmp/ingetrazo-parity-python'),(merged+'/usr/lib/x86_64-linux-gnu','/usr/lib/x86_64-linux-gnu'),(merged+work,work),(source,'/work'),('/tmp/.X11-unix','/tmp/.X11-unix'),(os.environ['XAUTHORITY'],'/auth')]:a+=['-v',x+':'+y+':ro']
a+=['-v',str(out)+':/out:rw','-e','DISPLAY='+os.environ['DISPLAY'],'-e','XAUTHORITY=/auth','-e','QT_QPA_PLATFORM=xcb','-e','HOME=/tmp/depth-home',image,'/tmp/ingetrazo-parity-python/bin/python',work+'/scripts/reference-gl-depth-resolve.py','/work/ingetrazo','/out']
start=time.monotonic()
try:
 with (out/'run.log').open('w') as f:r=subprocess.run(a,stdout=f,stderr=subprocess.STDOUT,timeout=60)
 (out/'transport.json').write_text(json.dumps({'returncode':r.returncode,'elapsed':time.monotonic()-start,'image_id':image},indent=2)+'\n');print('exit',r.returncode)
finally:subprocess.run(['podman','rm','--force',name],capture_output=True,timeout=10)
