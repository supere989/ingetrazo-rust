#!/usr/bin/env python3
"""Disposable X11 original capture, no user desktop/config mutation."""
import subprocess,os,tempfile,select,sys
with tempfile.TemporaryDirectory(prefix="transparent-gl-") as temporary:
 x=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1000x800x24","-nolisten","tcp"],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
 try:
  assert select.select([x.stdout],[],[],10)[0];display=x.stdout.readline().strip();env=dict(os.environ,DISPLAY=":"+display,QT_QPA_PLATFORM="xcb",LIBGL_ALWAYS_SOFTWARE="1",XDG_RUNTIME_DIR=temporary,XDG_CONFIG_HOME=temporary+"/config",XDG_CACHE_HOME=temporary+"/cache");env.pop("WAYLAND_DISPLAY",None)
  result=subprocess.run([sys.executable,"scripts/reference-transparent-gl-fixture.py",sys.argv[1]],env=env,timeout=60);raise SystemExit(result.returncode)
 finally:x.terminate();x.wait()
