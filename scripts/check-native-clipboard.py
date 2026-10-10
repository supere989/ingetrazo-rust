#!/usr/bin/env python3
"""Native X11 clipboard acceptance on a disposable server, never the desktop."""
import os,json,subprocess,tempfile,select
from pathlib import Path
from acceptance_context import context
def main():
 with tempfile.TemporaryDirectory(prefix="ingetrazo-clipboard-x11-") as t:
  server=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","800x600x24","-nolisten","tcp"],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  try:
   assert select.select([server.stdout],[],[],10)[0],"Xvfb did not allocate a display"
   display=server.stdout.readline().strip();assert display.isdigit(),display
   env=os.environ.copy();env["DISPLAY"]=":"+display;env.pop("WAYLAND_DISPLAY",None);env["XDG_RUNTIME_DIR"]=t
   command=["cargo","test","--locked","-p","ingetrazo-gui","--test","clipboard_transport_acceptance","--","--ignored","--nocapture"]
   result=subprocess.run(command,env=env,capture_output=True,text=True,timeout=180)
   report={"provenance":context(),"passed":result.returncode==0,"transport":"disposable X11 native clipboard ownership","command":command,"output":result.stdout+result.stderr,"scope":"Independent owners transfer geometry; unrelated text, malformed JSON and invalid topology clear cached geometry atomically; a replacement malformed request retires the previous valid preview. Wayland, Windows/macOS and clipboard-manager persistence are unverified."}
   Path("docs/audits/native-clipboard-semantic-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2));assert result.returncode==0
  finally:server.terminate();server.communicate(timeout=5)
if __name__=="__main__":main()
