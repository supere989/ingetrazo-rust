#!/usr/bin/env python3
"""Bounded owned Xvfb transport for original SKP/native IGZ render isolation."""
import os,sys,subprocess,select,tempfile,json,time
from pathlib import Path
root=Path(__file__).resolve().parents[1];out=Path(sys.argv[3]);out.mkdir(parents=True,exist_ok=True);cleanup=[]
def stop(p):
 p.terminate()
 try:p.wait(timeout=5)
 except subprocess.TimeoutExpired:
  cleanup.append('server terminate timeout');p.kill()
  try:p.wait(timeout=5)
  except subprocess.TimeoutExpired:cleanup.append('server kill timeout');raise
with tempfile.TemporaryDirectory(prefix='skp-qt-render-')as temporary:
 server=subprocess.Popen(['Xvfb','-displayfd','1','-screen','0','1000x800x24','-nolisten','tcp'],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
 try:
  assert select.select([server.stdout],[],[],10)[0];display=server.stdout.readline().strip();env=dict(os.environ,DISPLAY=':'+display,QT_QPA_PLATFORM='xcb',LIBGL_ALWAYS_SOFTWARE='1',XDG_RUNTIME_DIR=temporary,XDG_CONFIG_HOME=temporary+'/config',XDG_CACHE_HOME=temporary+'/cache');env.pop('WAYLAND_DISPLAY',None)
  command=[sys.executable,str(root/'scripts/reference-skp-real-render.py'),sys.argv[1],sys.argv[2],str(out)];started=time.monotonic()
  status={'command':command,'backend':'Qt/OpenGL llvmpipe on owned Xvfb','timed_out':False}
  try:
   with (out/'execution.log').open('w')as log:
    result=subprocess.run(command,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=90)
   status['returncode']=result.returncode
  except subprocess.TimeoutExpired:
   status.update(timed_out=True,returncode=124)
  finally:
   status['elapsed']=time.monotonic()-started
   (out/'transport.json').write_text(json.dumps(status,indent=2)+'\n')
  raise SystemExit(status['returncode'])
 finally:
  stop(server);(out/'cleanup.json').write_text(json.dumps({'bounded':True,'failures':cleanup})+'\n')
