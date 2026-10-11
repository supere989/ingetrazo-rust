#!/usr/bin/env python3
"""Bounded actual macOS Qt glyph/identity diagnostic; not the frozen GL frame oracle."""
import hashlib,json,os,platform,subprocess,sys
from pathlib import Path
if sys.platform!="darwin":raise SystemExit("Actual macOS required")
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
from PySide6.QtCore import QPointF,qVersion
from PySide6.QtGui import QColor,QFont,QFontInfo,QFontMetricsF,QImage,QPainter,QTextLayout
from PySide6.QtWidgets import QApplication
assert qVersion()=="6.12.0"
pin=subprocess.check_output(["git","-C","ingetrazo","rev-parse","HEAD"],text=True).strip()
assert pin=="6be29fe437a7cd992d4e079f71720821b25216b9"
app=QApplication([]);out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True);rows=[]
for requested in ["default","Sans Serif"]:
 for pixels in [13,17]:
  font=QFont() if requested=="default" else QFont(requested)
  font.setBold(True);font.setPixelSize(pixels)
  image=QImage(96,48,QImage.Format_RGB32);image.setDotsPerMeterX(round(96/0.0254));image.setDotsPerMeterY(round(96/0.0254));image.fill(0xffffffff)
  painter=QPainter(image);painter.setFont(font);painter.setPen(QColor("black"))
  painter.drawText(QPointF(10,30),"8");painter.end()
  info=QFontInfo(font);metrics=QFontMetricsF(font,image)
  layout=QTextLayout("8",font,image);layout.beginLayout();line=layout.createLine();line.setLineWidth(96);layout.endLayout()
  runs=[]
  for run in layout.glyphRuns():
   raw=run.rawFont();runs.append({"family":raw.familyName(),"style":raw.styleName(),"pixels":raw.pixelSize(),"glyphs":list(run.glyphIndexes()),"positions":[[p.x(),p.y()]for p in run.positions()],"head_sha256":hashlib.sha256(bytes(raw.fontTable("head"))).hexdigest(),"name_sha256":hashlib.sha256(bytes(raw.fontTable("name"))).hexdigest()})
  assert runs and metrics.horizontalAdvance("8")>0
  name=requested.replace(" ","-")+"-"+str(pixels)+".png";assert image.save(str(out/name))
  rows.append({"requested":requested,"pixels":pixels,"actual_family":info.family(),"actual_style":info.styleName(),"actual_bold":info.bold(),"actual_pixel_size":info.pixelSize(),"dpi":[image.logicalDpiX(),image.logicalDpiY()],"dpr":image.devicePixelRatio(),"advance":metrics.horizontalAdvance("8"),"ascent":metrics.ascent(),"descent":metrics.descent(),"glyph_runs":runs,"image":name,"image_sha256":hashlib.sha256((out/name).read_bytes()).hexdigest()})
(out/"qt-glyphs.json").write_text(json.dumps({"qt":qVersion(),"source_pin":pin,"platform":platform.platform(),"machine":platform.machine(),"scope":"Supplementary Qt RGB32 glyph/identity diagnostic; frozen Linux NVIDIA full frames remain strict authority","cases":rows},indent=2)+"\n")
