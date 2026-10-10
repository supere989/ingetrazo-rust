#!/usr/bin/env python3
"""Actual macOS Qt source painter and supplementary glyph-run identity observer.
Requires PySide6==6.12.0, fonttools, and original pinned submodule; no Fontconfig.
"""
import os,sys,json,hashlib,subprocess,platform
from pathlib import Path
if sys.platform!="darwin":raise SystemExit("Requires actual macOS; Linux substitutes are not evidence")
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
from PySide6.QtCore import qVersion,QRectF,QPointF,Qt
from PySide6.QtGui import QPainter,QImage,QFontInfo,QTextLayout,QTextOption
from PySide6.QtWidgets import QApplication
from fontTools.ttLib import TTFont,TTCollection
assert qVersion()=="6.12.0"
original=Path(os.environ.get("INGETRAZO_ORIGINAL","ingetrazo")).resolve()
pin=subprocess.check_output(["git","-C",str(original),"rev-parse","HEAD"],text=True).strip()
assert pin=="6be29fe437a7cd992d4e079f71720821b25216b9"
sys.path.insert(0,str(original))
from core.composition import TextoItem
from views.composer import paint_text_mm
app=QApplication([]);out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
cases=[("helvetica","Helvetica",False,False,"ABC"),("helvetica-bold","Helvetica",True,False,"ABC"),("helvetica-italic","Helvetica",False,True,"ABC"),("times-bold-italic","Times",True,True,"ABC"),("cjk","Helvetica",False,False,"漢字"),("arabic","Helvetica",False,False,"العربية")]
# Oracle-only disk inventory, never product renderer/font resolution.
files=[]
for directory in ["/System/Library/Fonts","/Library/Fonts",str(Path.home()/"Library/Fonts"),"/System/Library/PrivateFrameworks/FontServices.framework/Resources/Reserved"]:
 for p in Path(directory).rglob("*"):
  if p.suffix.lower() in (".ttf",".otf",".ttc",".otc") and p.is_file():files.append(p)
identities={}
inventory_errors=[]
for path in sorted(set(files)):
 try:
  if path.suffix.lower() in (".ttc",".otc"):
   owner=TTCollection(path,lazy=True);fonts=owner.fonts
  else:owner=TTFont(path,lazy=True);fonts=[owner]
  with path.open("rb") as stream:
   file_digest=hashlib.file_digest(stream,"sha256").hexdigest()
  for index,font in enumerate(fonts):
   # Read exact stored tables, independent of any fontTools decompilation.
   key=tuple(hashlib.sha256(font.reader[tag]).hexdigest() for tag in ("head","name"))
   identities.setdefault(key,[]).append({"path":str(path),"index":index,"postscript":font["name"].getDebugName(6),"sha256":file_digest})
  owner.close()
 except Exception as error:inventory_errors.append({"path":str(path),"error":repr(error)})
rows=[]
class Observe(QPainter):
 def drawText(self,rect,flags,text):
  font=self.font();option=QTextOption();option.setWrapMode(QTextOption.WordWrap);option.setAlignment(Qt.AlignmentFlag(flags&int(Qt.AlignHorizontal_Mask)))
  layout=QTextLayout(text,font);layout.setTextOption(option);layout.beginLayout();y=0
  while True:
   line=layout.createLine()
   if not line.isValid():break
   line.setLineWidth(rect.width());line.setPosition(QPointF(0,y));y+=line.height()
  layout.endLayout();runs=[]
  for run in layout.glyphRuns():
   raw=run.rawFont();key=tuple(hashlib.sha256(bytes(raw.fontTable(tag))).hexdigest()for tag in ["head","name"])
   physical=identities.get(key,[])
   # Retain missing identities and subsequent cases; fail only after writing the report.
   runs.append({"family":raw.familyName(),"style":raw.styleName(),"glyphs":list(run.glyphIndexes()),"positions":[[v.x(),v.y()]for v in run.positions()],"physical_faces":physical,"table_hashes":key,"raw_head_hex":bytes(raw.fontTable("head")).hex()})
  self.observed.append({"requested_font":{"family":font.family(),"bold":font.bold(),"italic":font.italic(),"weight":int(font.weight())},"actual_painter_font":QFontInfo(font).family(),"actual_painter_style":QFontInfo(font).styleName(),"actual_painter_traits":{"bold":QFontInfo(font).bold(),"italic":QFontInfo(font).italic(),"weight":int(QFontInfo(font).weight()),"exact_match":QFontInfo(font).exactMatch()},"supplementary_qtextlayout_runs":runs})
  return super().drawText(rect,flags,text)
for name,family,bold,italic,text in cases:
 image=QImage(800,300,QImage.Format_ARGB32_Premultiplied);image.fill(Qt.white)
 painter=Observe(image);painter.observed=[];painter.scale(3.,3.);painter.translate(10.37,10.23)
 paint_text_mm(painter,TextoItem(text=text,size_pt=14,family=family,bold=bold,italic=italic));painter.end()
 image.save(str(out/(name+".png")))
 rows.append({"name":name,"family":family,"bold":bold,"italic":italic,"text":text,"observations":painter.observed,"png_sha256":hashlib.sha256((out/(name+".png")).read_bytes()).hexdigest()})
assert len(rows)==6 and all(r["observations"] for r in rows)
report={"source_pin":pin,"qt":qVersion(),"platform":platform.platform(),"machine":platform.machine(),"script_sha256":hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),"cases":rows,"inventory_errors":inventory_errors,"scope":"Actual original paint_text_mm QImage pixels; supplementary Qt glyph-run matching is not the operative painter placement or native Canvas parity"}
(out/"original-macos-fonts.json").write_text(json.dumps(report,indent=2,ensure_ascii=False)+"\n")
missing=[r["name"] for r in rows if any(not run["physical_faces"] for call in r["observations"] for run in call["supplementary_qtextlayout_runs"])]
print(json.dumps({"captured":len(rows),"unmapped_cases":missing}))
if missing:raise SystemExit(1)
