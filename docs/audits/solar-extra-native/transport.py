import subprocess, pathlib, json, os, hashlib, time
q=lambda *a:subprocess.check_output(a,text=True).strip()
m=q('podman','inspect','ingetrazo-publish','--format','{{.GraphDriver.Data.MergedDir}}'); im=q('podman','inspect','ingetrazo-publish','--format','{{.Image}}')
src=pathlib.Path(m+os.environ.get('SOLAR_SOURCE','/tmp/ingetrazo-solar-source-nvidia-3'));out=pathlib.Path(os.environ.get('SOLAR_OUT','/tmp/solar-native-frozen'));out.mkdir(exist_ok=True)
(out/'icd.json').write_text('{"file_format_version":"1.0.0","ICD":{"library_path":"/usr/lib/libEGL_nvidia.so.0","api_version":"1.4.351"}}')
binary=m+'/tmp/ingetrazo-skp-native-target/debug/examples/native_vulkan_shadow_probe';meta={'binary_sha256':hashlib.sha256(pathlib.Path(binary).read_bytes()).hexdigest(),'source_commit':q('podman','exec','ingetrazo-publish','git','-C','/tmp/ingetrazo-vulkan-solar','rev-parse','HEAD'),'original_json_sha256':hashlib.sha256((src/'original.json').read_bytes()).hexdigest(),'cases':[]};(out/'transport.json').write_text(json.dumps(meta,indent=2))
for c in json.loads((src/'original.json').read_text())['cases']:
 if os.environ.get('SOLAR_CASE') and c['name']!=os.environ['SOLAR_CASE']:continue
 if os.environ.get('SOLAR_SKIP')==c['name']:continue
 name='ingetrazo-solar-native-'+str(os.getpid());dest=out/c['name'];dest.mkdir(exist_ok=True)
 cmd=['podman','run','--rm','--name',name,'--read-only','--network','none','--device','nvidia.com/gpu=0','--userns','keep-id:uid=1000,gid=1000','--user','1000:1000','--security-opt','label=disable','--tmpfs','/tmp:rw','-v','/usr/lib/libEGL.so.1:/usr/lib/libEGL.so.1:ro','-v','/usr/lib/libGLdispatch.so.0:/usr/lib/libGLdispatch.so.0:ro','-v',binary+':/probe:ro','-v',str(src)+':/input:ro','-v',str(out)+':/out:rw','-e','VK_DRIVER_FILES=/out/icd.json',im,'/probe','/input/'+c['igz'],'/out/'+c['name'],'--reference','/input/original.json',c['name']]
 row={'case':c['name'],'command':cmd};start=time.monotonic()
 try:
  with (dest/'runtime.log').open('w') as f:row['exit']=subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT,timeout=60).returncode
 except subprocess.TimeoutExpired:row['exit']=124
 finally:subprocess.run(['podman','rm','--force',name],capture_output=True,timeout=10)
 row['elapsed']=time.monotonic()-start;meta['cases'].append(row);(out/'transport.json').write_text(json.dumps(meta,indent=2))
 print(c['name'],row['exit'],flush=True)
 if row['exit']:break
