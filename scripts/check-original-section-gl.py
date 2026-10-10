#!/usr/bin/env python3
"""Capture original stencil behavior on a disposable X11 display, not physical expectations."""
import subprocess,os,tempfile,select,sys
with tempfile.TemporaryDirectory(prefix="section-gl-") as t:
 p=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1000x800x24","-nolisten","tcp"],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
 try:
  assert select.select([p.stdout],[],[],10)[0];d=p.stdout.readline().strip();e=os.environ.copy();e.update(DISPLAY=":"+d,QT_QPA_PLATFORM="xcb",LIBGL_ALWAYS_SOFTWARE="1",XDG_RUNTIME_DIR=t,XDG_CONFIG_HOME=t+"/config",XDG_CACHE_HOME=t+"/cache");e.pop("WAYLAND_DISPLAY",None)
  r=subprocess.run([sys.executable,"scripts/reference-section-gl-fixture.py","docs/audits/section-original-gl"],env=e,timeout=60);raise SystemExit(r.returncode)
 finally:p.terminate();p.wait()

