#!/usr/bin/env python3
"""Actual original paint_sheet_border_mm vs native source span geometry; strict zero pixels."""
import sys,json,subprocess,hashlib,os
from pathlib import Path
from types import SimpleNamespace
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QImage,QPainter,QColor
from PySide6.QtCore import Qt
app=QApplication([]);assert __import__('PySide6.QtCore',fromlist=['qVersion']).qVersion()=='6.12.0'
root=Path(__file__).resolve().parents[1];sys.path.insert(0,'/work/ingetrazo')
from views.composer import paint_sheet_border_mm
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd='/work/ingetrazo',text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9'
out=root/'docs/audits/paper-canvas-border';recipe=out/'recipes.json';cases=json.loads(recipe.read_text())
expected=['sharp-solid','rounded-solid','sharp-double','rounded-double','thin-sharp','thin-rounded','clamped-elliptical','dashed-retained'];assert [c['name']for c in cases]==expected
binary=Path(os.environ['CARGO_TARGET_DIR'])/'debug/examples/paper_border_span_fixture'
native=json.loads(subprocess.check_output([str(binary)],input=recipe.read_bytes()));assert [c['name']for c in native]==expected
results=[]
for c,n in zip(cases,native):
 a=QImage(160,100,QImage.Format_ARGB32_Premultiplied);a.fill(Qt.white);p=QPainter(a);p.setRenderHints(QPainter.Antialiasing|QPainter.SmoothPixmapTransform);p.translate(*c['phase']);p.scale(c['density'],c['density'])
 model=SimpleNamespace(border=True,page_size_mm=lambda:c['page'],margin_mm=c['margin'],border_mm=c['width'],border_radius_mm=c['radius'],border_color='#000000',border_style=c['style']);paint_sheet_border_mm(p,model);p.end();a.save(str(out/(c['name']+'-source.png')))
 if 'error'in n:results.append(dict(name=c['name'],native_error=n['error'],strict=False));continue
 b=QImage(160,100,QImage.Format_ARGB32_Premultiplied);b.fill(Qt.white)
 for stroke in n['strokes']:
  for x,y,length,coverage in stroke:
   for xx in range(max(0,x),min(b.width(),x+length)):
    if 0<=y<b.height():
     old=b.pixelColor(xx,y).red();t=old*(255-coverage);v=(t+(t>>8)+128)>>8;b.setPixelColor(xx,y,QColor(v,v,v))
 b.save(str(out/(c['name']+'-native.png')))
 diff=[(x,y,a.pixelColor(x,y).red(),b.pixelColor(x,y).red())for y in range(a.height())for x in range(a.width())if a.pixel(x,y)!=b.pixel(x,y)]
 results.append(dict(name=c['name'],changed=len(diff),first=diff[:5],strict=not diff))
report=dict(original_pin=subprocess.check_output(['git','rev-parse','HEAD'],cwd='/work/ingetrazo',text=True).strip(),recipes_sha256=hashlib.sha256(recipe.read_bytes()).hexdigest(),binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),scope='Actual original border painter vs native CPU span geometry; rendered native canvas/cache still separate',cases=results)
(out/'span-results.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(results,indent=2));sys.exit(0 if all(c.get('strict')for c in results)else 1)
