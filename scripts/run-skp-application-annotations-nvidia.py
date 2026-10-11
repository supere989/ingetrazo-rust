#!/usr/bin/env python3
"""Read-only disposable CDI transport; only the created application windows are captured."""
import os,sys,json,time,subprocess,hashlib
from pathlib import Path
out=Path(sys.argv[1]);assert not out.exists();out.mkdir(mode=0o700)
query=lambda *a:subprocess.check_output(a,text=True).strip()
merged=query('podman','inspect','ingetrazo-publish','--format','{{.GraphDriver.Data.MergedDir}}');image=query('podman','inspect','ingetrazo-publish','--format','{{.Image}}')
source=query('podman','inspect','ingetrazo-publish','--format','{{range .Mounts}}{{if eq .Destination "/work"}}{{.Source}}{{end}}{{end}}')
work='/tmp/ingetrazo-skp-native-import';binary='/tmp/ingetrazo-skp-native-target/debug/examples/native_vulkan_annotation_overlay_fixture';inputs='/tmp/annotation-real-app-input';auth=Path(os.environ['XAUTHORITY']);assert auth.is_file()
meta=dict(source=query('podman','exec','ingetrazo-publish','git','-C',work,'rev-parse','HEAD'),image_id=image,binary_sha256=hashlib.sha256(Path(merged+binary).read_bytes()).hexdigest(),runs=[],scope='integral bounds production render_viewport and actual IngeTrazoApp.prepare_gpu; full application chrome remains separately required')
name='ingetrazo-annotation-paired-'+str(os.getpid())
base=['podman','run','--rm','--name',name,'--read-only','--network','none','--device','nvidia.com/gpu=0','--userns','keep-id:uid=1000,gid=1000','--user','1000:1000','--security-opt','label=disable','--tmpfs','/tmp:rw','-v','/usr/lib/libEGL.so.1:/usr/lib/libEGL.so.1:ro','-v','/usr/lib/libGLdispatch.so.0:/usr/lib/libGLdispatch.so.0:ro','-v',merged+'/usr/share:/usr/share:ro','-v',merged+'/etc/fonts:/etc/fonts:ro','-v',merged+'/usr/lib/x86_64-linux-gnu:/usr/lib/x86_64-linux-gnu:ro','-v',merged+'/tmp/ingetrazo-parity-python:/tmp/ingetrazo-parity-python:ro','-v',merged+work+':'+work+':ro','-v',source+':/work:ro','-v',merged+'/tmp/ingetrazo-openskp-reference:/oracle:ro','-v',merged+inputs+':/input:ro','-v',merged+binary+':/probe:ro','-v',str(out)+':/out:rw','-v','/tmp/.X11-unix:/tmp/.X11-unix:ro','-v',str(auth)+':/auth:ro','-e','DISPLAY='+os.environ['DISPLAY'],'-e','XAUTHORITY=/auth','-e','QT_QPA_PLATFORM=xcb','-e','QT_OPENGL=desktop','-e','HOME=/tmp/home','-e','XDG_CONFIG_HOME=/tmp/config','-e','XDG_CACHE_HOME=/tmp/cache']
def run(label,args):
 started=time.monotonic();row=dict(label=label,command=args,timeout_seconds=60)
 try:
  with(out/(label+'.log')).open('w')as stream:row['returncode']=subprocess.run(args,stdout=stream,stderr=subprocess.STDOUT,timeout=60).returncode
 except subprocess.TimeoutExpired:row.update(returncode=124,timed_out=True)
 finally:
  row['elapsed']=time.monotonic()-started
  try:row['cleanup_returncode']=subprocess.run(['podman','rm','--force',name],capture_output=True,timeout=10).returncode
  except subprocess.TimeoutExpired:row['cleanup_timed_out']=True
  meta['runs'].append(row);(out/'transport.json').write_text(json.dumps(meta,indent=2)+'\n')
 return row['returncode']
(out/'nvidia-egl-icd.json').write_text('{"file_format_version":"1.0.0","ICD":{"library_path":"/usr/lib/libEGL_nvidia.so.0","api_version":"1.4.351"}}')
for model in ['gondola_v20','capilla_quiroz_v17']:
 for view in ['iso','asymmetric']:
  case=model+'-'+view;(out/case).mkdir()
  args=base+['-e','INGETRAZO_VULKAN_DEVICE=NVIDIA','-e','VK_DRIVER_FILES=/out/nvidia-egl-icd.json',image,'/tmp/ingetrazo-parity-python/bin/python',work+'/scripts/capture-owned-native-skp-annotations.py','/probe','/input/'+case+'-camera.json','/input/'+model+'.igz','/out/'+case+'/native']
  if run(case+'-native',args):continue
  args=base+[image,'/tmp/ingetrazo-parity-python/bin/python',work+'/scripts/reference-skp-application-annotations.py','/work/ingetrazo','/oracle','/oracle/packages/python/tests/fixtures/'+model+'.skp','/out/'+case+'/native','/out/'+case+'/original']
  run(case+'-original',args)
assert hashlib.sha256(Path(merged+binary).read_bytes()).hexdigest()==meta['binary_sha256'],'Executed binary changed during paired run'
raise SystemExit(0 if all(r['returncode']==0 for r in meta['runs'])and len(meta['runs'])==8 else 1)
