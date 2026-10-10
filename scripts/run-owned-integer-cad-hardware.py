#!/usr/bin/env python3
"""Bounded GPU0, read-only devbox; capture only its newly created own window.
Run this transport on the host with copied immutable build artifacts. It does
not edit a repository, change authentication, or capture desktop/root pixels.
"""
import argparse,hashlib,json,os,subprocess,time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--binary',type=Path,required=True);p.add_argument('--capture-helper',type=Path,required=True);p.add_argument('--source',type=Path,required=True);p.add_argument('--runtime',type=Path,required=True);p.add_argument('--publish-libs',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--image',required=True);p.add_argument('--revision',required=True);p.add_argument('--xauth',type=Path,required=True);p.add_argument('--display',default=os.environ.get('DISPLAY',':0'));p.add_argument('--mode',choices=['shapes','lifecycle'],required=True);args=p.parse_args()
for value in [args.binary,args.capture_helper,args.source,args.runtime,args.publish_libs,args.xauth]:assert value.exists(),value
image=json.loads(subprocess.check_output(['podman','image','inspect',args.image],text=True))[0]['Id']
args.output.mkdir(parents=True,exist_ok=False)
icd=args.output/'nvidia-glx-icd.json';icd.write_text(json.dumps({'file_format_version':'1.0.0','ICD':{'library_path':'libGLX_nvidia.so.0','api_version':'1.4.351'}}))
variants={'shapes':{}} if args.mode=='shapes' else {'baseline':{},'callback-only':{'INGETRAZO_CAD2D_CALLBACK_ONLY':'1'},'callback-error':{'INGETRAZO_CAD2D_CALLBACK_ERROR':'1'},'pressure':{'INGETRAZO_CAD2D_PRESSURE':'1'},'uniform':{'INGETRAZO_CAD2D_UNIFORM_UPDATE':'1'}}
results=[]
for label,extra in variants.items():
 out=args.output/label;out.mkdir();capture=out/'capture'
 if args.mode=='shapes':capture.mkdir()
 name=f'ingetrazo-integer-cad-{os.getpid()}-{label}'
 cmd=['podman','run','--rm','--name',name,'--userns=keep-id','--user',f'{os.getuid()}:{os.getgid()}','--device=nvidia.com/gpu=0','--network=none','--read-only','--security-opt','label=disable','--tmpfs','/tmp:rw,size=256m']
 for host,target in [('/tmp/.X11-unix','/tmp/.X11-unix'),(args.xauth,'/xauth'),(args.binary,'/proof/probe'),(args.capture_helper,'/proof/check.py'),(args.source,'/source'),(args.runtime,'/runtime'),(args.publish_libs,'/publish-libs'),('/usr/share/X11/xkb','/usr/share/X11/xkb'),(icd,'/icd.json')]:cmd+=['-v',f'{Path(host).resolve()}:{target}:ro']
 cmd+=['-v',f'{out.resolve()}:/out:rw']
 env={'DISPLAY':args.display,'XAUTHORITY':'/xauth','VK_ICD_FILENAMES':'/icd.json','LD_LIBRARY_PATH':'/runtime:/publish-libs','INGETRAZO_VULKAN_DEVICE':'NVIDIA','INGETRAZO_CAD2D_INTEGER_BLEND':'1','INGETRAZO_SHAPE_DIRECT':'1','INGETRAZO_SHAPE_RECIPES':'/source/recipes.json','INGETRAZO_SHAPE_OUT':'/out/capture',**extra}
 for key,value in env.items():cmd+=['-e',f'{key}={value}']
 cmd +=[image,'python3','/proof/check.py','/proof/probe','/out/capture']
 start=time.monotonic();result={'case':label,'image_id':image,'revision':args.revision,'binary_sha256':hashlib.sha256(args.binary.read_bytes()).hexdigest(),'helper_sha256':hashlib.sha256(args.capture_helper.read_bytes()).hexdigest(),'command':cmd,'timeout_s':45}
 try:
  run=subprocess.run(cmd,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=45);result['exit_code']=run.returncode;(out/'transport.log').write_text(run.stdout)
 except subprocess.TimeoutExpired as e:
  data=e.stdout or b'';(out/'transport.log').write_text(data.decode(errors='replace') if isinstance(data,bytes) else data);result.update(exit_code=124,timed_out=True)
 finally:
  try:
   clean=subprocess.run(['podman','rm','-f',name],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=10);result['cleanup_exit_code']=clean.returncode
  except subprocess.TimeoutExpired:result['cleanup_timed_out']=True
  result['elapsed_s']=time.monotonic()-start;(out/'transport.json').write_text(json.dumps(result,indent=2)+'\n');results.append(result);(args.output/'transport-results.json').write_text(json.dumps(results,indent=2)+'\n')
 assert result['exit_code']==0,(label,result['exit_code'],'retained transport failure')
print(json.dumps({'passed':True,'cases':list(variants),'image_id':image}))
