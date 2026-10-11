#!/usr/bin/env python3
"""Host transport, run under podman unshare; only a verified owned Xwayland seat."""
import hashlib,json,os,subprocess,sys,time
from pathlib import Path
work='/tmp/ingetrazo-hidden-ghost';out=Path(sys.argv[1]);assert not out.exists();out.mkdir(mode=0o700)
seat=Path('/run/user/1000/input-director/ingetrazo-sweep');state=json.loads((seat/'session.json').read_text());pid=state['kwin']['pid']
locks=[p for p in Path('/tmp').glob('.X*-lock')if p.read_text().strip()==str(pid)];assert len(locks)==1
number=locks[0].name[2:-5];display=':'+number
# KWin starts its own Xwayland on the first connection to its reserved socket.
import socket
sock=socket.socket(socket.AF_UNIX);sock.settimeout(3);sock.connect('/tmp/.X11-unix/X'+number);time.sleep(.3);sock.close()
children=Path(f'/proc/{pid}/task/{pid}/children').read_text().split();assert any(('Xwayland\\x00'+display+'\\x00').encode().replace(b'\\x00',b'\x00') in Path(f'/proc/{c}/cmdline').read_bytes()for c in children)
auth=seat/'oracle-empty.auth';auth.write_bytes(b'');auth.chmod(0o600)
def query(*a):return subprocess.check_output(a,text=True).strip()
merged=query('podman','inspect','ingetrazo-publish','--format','{{.GraphDriver.Data.MergedDir}}');image=query('podman','inspect','ingetrazo-publish','--format','{{.Image}}');source=query('podman','inspect','ingetrazo-publish','--format','{{range .Mounts}}{{if eq .Destination "/work"}}{{.Source}}{{end}}{{end}}')
name='ingetrazo-sweep-oracle-'+str(os.getpid())
args=['podman','run','--rm','--name',name,'--read-only','--network','none','--device','nvidia.com/gpu=0','--userns','keep-id:uid=1000,gid=1000','--user','1000:1000','--security-opt','label=disable','--tmpfs','/tmp:rw']
for left,right in [('/usr/lib/libEGL.so.1','/usr/lib/libEGL.so.1'),('/usr/lib/libGLdispatch.so.0','/usr/lib/libGLdispatch.so.0'),(merged+'/tmp/ingetrazo-parity-python','/tmp/ingetrazo-parity-python'),(merged+'/usr/lib/x86_64-linux-gnu','/usr/lib/x86_64-linux-gnu'),(merged+work,work),(source,'/work'),('/tmp/.X11-unix','/tmp/.X11-unix'),(str(auth),'/auth')]:args+=['-v',left+':'+right+':ro']
args+=['-v',str(out)+':/out:rw','-v',merged+'/tmp/face-sweep-original-recipes.json:/recipes.json:ro']
for entry in ['DISPLAY='+display,'XAUTHORITY=/auth','QT_QPA_PLATFORM=xcb','QT_OPENGL=desktop','__GLX_VENDOR_LIBRARY_NAME=nvidia','__NV_PRIME_RENDER_OFFLOAD=1','HOME=/tmp/solar-home','XDG_CONFIG_HOME=/tmp/solar-config','XDG_CACHE_HOME=/tmp/solar-cache']:args+=['-e',entry]
if os.environ.get('INGETRAZO_SOLAR_EXTRA_ONLY'):args+=['-e','INGETRAZO_SOLAR_EXTRA_ONLY=1']
args += [image,'/tmp/ingetrazo-parity-python/bin/python',work+'/scripts/reference-face-sweep-render.py','/recipes.json','/out']
start=time.monotonic();rc=124
try:
 with(out/'original.log').open('w')as f:rc=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT,timeout=60).returncode
finally:
 subprocess.run(['podman','rm','--force',name],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=10)
 meta={'image':image,'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','seat':state['name'],'seat_kwin_pid':pid,'display':display,'command':args,'elapsed_seconds':time.monotonic()-start,'returncode':rc,'oracle_sha256':hashlib.sha256(Path(merged+work+'/scripts/reference-face-sweep-render.py').read_bytes()).hexdigest()};(out/'transport.json').write_text(json.dumps(meta,indent=2)+'\n');print(json.dumps({'returncode':rc,'elapsed_seconds':meta['elapsed_seconds']}))
raise SystemExit(rc)
