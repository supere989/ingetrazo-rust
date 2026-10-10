#!/usr/bin/env python3
import importlib.util,json,shutil,sys,tempfile
from pathlib import Path
from PySide6.QtGui import QImage,QColor
spec=importlib.util.spec_from_file_location("checker",Path(__file__).with_name("check-native-image-outlines.py"));checker=importlib.util.module_from_spec(spec);spec.loader.exec_module(checker)
source,native=map(Path,sys.argv[1:3]);checker.check(source,native);results=[]
with tempfile.TemporaryDirectory(prefix="image-outline-checker-negative-")as tmp:
 root=Path(tmp)
 for mutation in ["pixel","missing-case","wrong-vendor","software-type","source-ablation","wrong-engine"]:
  a=root/"source";b=root/"native";shutil.copytree(source,a);shutil.copytree(native,b)
  p=b/"native-ui-results.json";report=json.loads(p.read_text())
  if mutation=="pixel":
   image=QImage(str(b/"image-quadrants.png"));c=image.pixelColor(400,300);image.setPixelColor(400,300,QColor((c.red()+1)%256,c.green(),c.blue(),c.alpha()));assert image.save(str(b/"image-quadrants.png"))
  elif mutation=="missing-case":report["cases"].pop()
  elif mutation=="wrong-vendor":report["cases"][0]["renderer_device"]["vendor_id"]=0x1002
  elif mutation=="software-type":report["cases"][0]["renderer_device"]["device_type"]="CPU"
  else:
   q=a/"original-alpha-results.json";s=json.loads(q.read_text())
   if mutation=="source-ablation":s["cases"][0]["image_outline_ablation"]=True
   else:s["cases"][0]["outline_paint_metadata"]["paint_engine"]=9
   q.write_text(json.dumps(s))
  p.write_text(json.dumps(report))
  try:checker.check(a,b)
  except AssertionError:results.append({"mutation":mutation,"rejected":True})
  else:raise AssertionError("Mutation accepted: "+mutation)
  shutil.rmtree(a);shutil.rmtree(b)
print(json.dumps({"passed":True,"negative_controls":results},indent=2))
