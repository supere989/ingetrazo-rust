#!/usr/bin/env python3
"""Host transport only; exact pinned Qt and Vulkan execute in readonly CDI devboxes.
Run under podman unshare so readonly overlay paths are visible. Own Qt windows only.
"""
import os,sys,json,time,subprocess,hashlib
from pathlib import Path
publish='ingetrazo-publish';work=os.environ.get('INGETRAZO_SKP_WORKTREE','/tmp/ingetrazo-skp-native-import');out=Path(sys.argv[1])
assert Path(work).is_absolute(), 'absolute container worktree required'
assert not out.exists(),'fresh output required';out.mkdir(mode=0o700)
def query(*a):return subprocess.check_output(a,text=True).strip()
merged=query('podman','inspect',publish,'--format','{{.GraphDriver.Data.MergedDir}}');image=query('podman','inspect',publish,'--format','{{.Image}}')
source=query('podman','inspect',publish,'--format','{{range .Mounts}}{{if eq .Destination "/work"}}{{.Source}}{{end}}{{end}}')
assert query('podman','exec',publish,'git','-C','/work/ingetrazo','rev-parse','HEAD')=='6be29fe437a7cd992d4e079f71720821b25216b9'
display=os.environ['DISPLAY'];auth=Path(os.environ['XAUTHORITY']);assert auth.is_file()
importer='/tmp/ingetrazo-skp-runtime-metadata-importer';probe=os.environ.get('INGETRAZO_SKP_NATIVE_PROBE','/tmp/ingetrazo-skp-world-aligned-probe')
for f in [importer,probe]:assert Path(merged+f).is_file()
source_sha=query('podman','exec',publish,'git','-C',work,'rev-parse','HEAD')
checkout_status=query('podman','exec',publish,'git','-C',work,'status','--porcelain','--untracked-files=no','--ignore-submodules=all')
probe_source=os.environ.get('INGETRAZO_SKP_PROBE_SOURCE')
if probe_source:
 assert len(probe_source)==40 and all(c in '0123456789abcdef' for c in probe_source), 'full frozen probe source SHA required'
meta={'image_id':image,'source':source_sha,'importer_sha256':hashlib.sha256(Path(merged+importer).read_bytes()).hexdigest(),'probe_sha256':hashlib.sha256(Path(merged+probe).read_bytes()).hexdigest(),'device':'nvidia.com/gpu=0','cases':[]}
meta['source_provenance']={'checkout_path':work,'checkout_head':source_sha,'tracked_changes':checkout_status.splitlines(),'probe_build_source':probe_source,'scope':'Checkout HEAD is an observation, not proof of the probe build source. Probe SHA256 identifies executed bytes; supplied build source must be backed by retained build evidence.'}
name='ingetrazo-skp-hardware-'+str(os.getpid())
common=['podman','run','--rm','--name',name,'--read-only','--network','none','--device','nvidia.com/gpu=0','--userns','keep-id:uid=1000,gid=1000','--user','1000:1000','--security-opt','label=disable','--tmpfs','/tmp:rw','-v','/usr/lib/libEGL.so.1:/usr/lib/libEGL.so.1:ro','-v','/usr/lib/libGLdispatch.so.0:/usr/lib/libGLdispatch.so.0:ro']
def run(label,args,timeout):
 started=time.monotonic();row={'label':label,'command':args,'timeout_seconds':timeout,'timed_out':False}
 try:
  with(out/(label+'.log')).open('w')as log:
   result=subprocess.run(args,stdout=log,stderr=subprocess.STDOUT,timeout=timeout)
  row['returncode']=result.returncode
 except subprocess.TimeoutExpired:row.update(returncode=124,timed_out=True)
 finally:
  row['elapsed']=time.monotonic()-started;meta['cases'].append(row);(out/'transport.json').write_text(json.dumps(meta,indent=2)+'\n')
  cleanup=subprocess.run(['podman','rm','--force',name],capture_output=True,text=True,timeout=10);row['cleanup_returncode']=cleanup.returncode
 return row['returncode']
original=Path(sys.argv[2]) if len(sys.argv)>2 else out/'original'
if len(sys.argv)<=2:original.mkdir(mode=0o700)
qt=common+['-v',merged+'/tmp/ingetrazo-parity-python:/tmp/ingetrazo-parity-python:ro','-v',merged+'/usr/lib/x86_64-linux-gnu:/usr/lib/x86_64-linux-gnu:ro','-v',merged+work+':'+work+':ro','-v',source+':/work:ro','-v',merged+'/tmp/ingetrazo-openskp-reference:/tmp/ingetrazo-openskp-reference:ro','-v',merged+importer+':'+importer+':ro','-v',str(original)+':/out:rw','-v','/tmp/.X11-unix:/tmp/.X11-unix:ro','-v',str(auth)+':/auth:ro','-e','DISPLAY='+display,'-e','XAUTHORITY=/auth','-e','QT_QPA_PLATFORM=xcb','-e','QT_OPENGL=desktop','-e','INGETRAZO_REQUIRE_NVIDIA=1','-e','HOME=/tmp/skp-home','-e','XDG_CONFIG_HOME=/tmp/skp-config','-e','XDG_CACHE_HOME=/tmp/skp-cache',image,'/tmp/ingetrazo-parity-python/bin/python',work+'/scripts/reference-skp-real-render.py','/tmp/ingetrazo-openskp-reference',importer,'/out']
if len(sys.argv)<=2:
 original_status=run('original-nvidia',qt,90)
else:
 reference_results=json.loads((original/'qt-isolation-results.json').read_text())
 assert len(reference_results['cases'])==4
 assert all('NVIDIA GeForce RTX 5090' in c['gl_contexts'][0]['gl']['renderer']for c in reference_results['cases'])
 original_status=0 if reference_results['passed']else 1
 meta['retained_reference']={'path':str(original),'strict_returncode':original_status,'files':{f.name:hashlib.sha256(f.read_bytes()).hexdigest()for f in original.iterdir()if f.is_file()}}
if not (original/'protocol.json').is_file():raise SystemExit(original_status or 1)

native=out/'native';native.mkdir(mode=0o700);(native/'nvidia-egl-icd.json').write_text('{"file_format_version":"1.0.0","ICD":{"library_path":"/usr/lib/libEGL_nvidia.so.0","api_version":"1.4.351"}}\n')
command=common+['-v',merged+probe+':/probe:ro','-v',str(original)+':/input:ro','-v',str(native)+':/out:rw','-e','VK_DRIVER_FILES=/out/nvidia-egl-icd.json',image,'/probe','/input','/out']
if run('native-nvidia',command,90):raise SystemExit(1)
(out/'transport.json').write_text(json.dumps(meta,indent=2)+'\n')
raise SystemExit(original_status)
