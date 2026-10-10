#!/usr/bin/env python3
"""Native production section painter versus actual original stencil captures."""
import subprocess,os,tempfile,select,json
from pathlib import Path
from PySide6.QtGui import QImage
from acceptance_context import context
def main():
 subprocess.run(["cargo","build","--locked","-p","ingetrazo-gui","--example","section_render_acceptance_fixture"],check=True)
 root=Path("docs/audits/section-rust-render");root.mkdir(exist_ok=True);rows=[]
 exe=os.environ.get("INGETRAZO_SECTION_FIXTURE",str(Path(os.environ.get("CARGO_TARGET_DIR","target"))/"debug/examples/section_render_acceptance_fixture"))
 reference=json.loads(Path("docs/audits/section-original-gl/original-gl-results.json").read_text())
 with tempfile.TemporaryDirectory(prefix="section-native-") as t:
  p=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1000x800x24","-nolisten","tcp"],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
  try:
   assert select.select([p.stdout],[],[],10)[0];display=p.stdout.readline().strip()
   e=os.environ.copy();e.update(DISPLAY=":"+display,LIBGL_ALWAYS_SOFTWARE="1",XDG_RUNTIME_DIR=t,XDG_CONFIG_HOME=t+"/config",XDG_CACHE_HOME=t+"/cache");e.pop("WAYLAND_DISPLAY",None)
   for r in reference["cases"]:
    path=(root/(r["case"]+".png")).resolve();path.unlink(missing_ok=True)
    subprocess.run([exe,r["case"],str(path)],env=e,check=True,timeout=30,capture_output=True)
    image=QImage(str(path));assert not image.isNull();assert image.width()==800 and image.height()==600
    count=sum(image.pixelColor(x,y).red()>180 and image.pixelColor(x,y).blue()>180 and image.pixelColor(x,y).green()<80 for y in range(600) for x in range(800))
    expected=r["magenta_fill_pixels"];same=(count==0 if expected==0 else abs(count-expected)/expected<.05)
    rows.append({"case":r["case"],"original_magenta_pixels":expected,"rust_magenta_pixels":count,"coverage_within_five_percent":same})
  finally:p.terminate();p.wait()
 report={"provenance":context(),"passed":all(r["coverage_within_five_percent"] for r in rows),"cases":rows,"scope":"Actual native eframe production painter screenshot, equal800x600 Top/Iso/Front orthographic cameras and authored two-storey geometry plus coincident cut/backface versus real originalQtGL stencil. Five-percent cap coverage tolerance for raster/contour antialiasing; zero-fill cases exact. No full scene UI gesture or camera-generalization claim."}
 (root/"results.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2));assert report["passed"],rows
if __name__=="__main__":main()
