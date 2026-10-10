import os,subprocess,sys
from pathlib import Path
binary,case=sys.argv[1:]
base=Path('/tmp/ingetrazo-resize-transaction-hardware')
merged=subprocess.check_output(['podman','inspect','ingetrazo-publish','--format','{{.GraphDriver.Data.MergedDir}}'],text=True).strip()
image=subprocess.check_output(['podman','inspect','ingetrazo-publish','--format','{{.Image}}'],text=True).strip()
cmd=['podman','run','--rm','--name','ingetrazo-resize-proof','--read-only','--network','none','--device','nvidia.com/gpu=0','--userns','keep-id:uid=1000,gid=1000','--user','1000:1000','--security-opt','label=disable','--tmpfs','/tmp:rw']
mounts=[(str(base/'bin'/binary),'/probe-bin','ro'),(str(base/'out'),'/out','rw'),(merged+'/tmp/ingetrazo-parity-python','/tmp/ingetrazo-parity-python','ro'),(merged+'/usr/lib/x86_64-linux-gnu','/usr/lib/x86_64-linux-gnu','ro'),(merged+'/usr/share/X11/xkb','/usr/share/X11/xkb','ro'),(merged+'/tmp/ingetrazo-resize-transaction/scripts','/scripts','ro'),('/usr/lib/libEGL.so.1','/usr/lib/libEGL.so.1','ro'),('/usr/lib/libGLdispatch.so.0','/usr/lib/libGLdispatch.so.0','ro'),('/tmp/.X11-unix','/tmp/.X11-unix','ro'),(os.environ['XAUTHORITY'],'/auth','ro')]
for src,dst,mode in mounts:cmd+=['-v',f'{src}:{dst}:{mode}']
for env in ['DISPLAY','XAUTHORITY=/auth','QT_QPA_PLATFORM=xcb','WINIT_UNIX_BACKEND=x11','INGETRAZO_VULKAN_DEVICE=NVIDIA','INGETRAZO_CAD2D_INTEGER_BLEND=1','VK_DRIVER_FILES=/out/nvidia-glx-icd.json']:cmd+=['-e',env]
cmd += [image,'/tmp/ingetrazo-parity-python/bin/python','/scripts/check-owned-native-cad2d.py','/probe-bin','/out/'+case,'12']
r=subprocess.run(cmd,timeout=40);sys.exit(r.returncode)
