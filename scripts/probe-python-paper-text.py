#!/usr/bin/env python3
"""Actual source text painter PDFs plus Qt font/layout observations, no native parity claim."""
import os,sys,json,hashlib,subprocess,struct
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QPainter,QPdfWriter,QFontInfo,QFontMetricsF,QRawFont,QTextLayout,QTextOption,QPageSize
from PySide6.QtCore import QRectF,QSizeF,Qt,QPointF
from core.composition import TextoItem,RENDER_DPI,PT_TO_MM
from views.composer import paint_text_mm, _text_metrics, _line_count, text_bg_span_mm
app=QApplication([]);out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
class Observe(QPainter):
 def drawText(self,rect,flags,text):
  f=self.font();info=QFontInfo(f);m=QFontMetricsF(f);raw=QRawFont.fromFont(f)
  resolved=subprocess.check_output(["fc-match","-f","%{file}",info.family()+":style="+info.styleName()],text=True)
  data=Path(resolved).read_bytes();tables={}
  for i in range(struct.unpack_from(">H",data,4)[0]):
   tag,_,offset,size=struct.unpack_from(">4sIII",data,12+16*i);tables[tag]=data[offset:offset+size]
  assert bytes(raw.fontTable("head"))==tables[b"head"],"fc-match font differs from actual QRawFont"
  matrix=self.transform()
  row={"text":text,"rect":[rect.x(),rect.y(),rect.width(),rect.height()],"flags":flags,"transform":[matrix.m11(),matrix.m12(),matrix.m21(),matrix.m22(),matrix.dx(),matrix.dy()],"font":{"requested":f.family(),"resolved":info.family(),"style":info.styleName(),"pixel_size":f.pixelSize(),"bold":f.bold(),"italic":f.italic(),"underline":f.underline(),"ascent":m.ascent(),"descent":m.descent(),"leading":m.leading(),"height":m.height(),"advance":m.horizontalAdvance(text),"file":resolved,"sha256":hashlib.sha256(Path(resolved).read_bytes()).hexdigest(),"head_table_sha256":hashlib.sha256(bytes(raw.fontTable("head"))).hexdigest(),"units_per_em":raw.unitsPerEm()},"raw_glyph_indices":raw.glyphIndexesForString(text)}
  # Supplementary actual Qt QTextLayout model; do not call it source QPainter placement.
  option=QTextOption();option.setWrapMode(QTextOption.WrapMode.WordWrap);option.setAlignment(Qt.AlignmentFlag(flags & int(Qt.AlignHorizontal_Mask)))
  layout=QTextLayout(text,f);layout.setTextOption(option);layout.beginLayout();lines=[];y=0.
  while True:
   line=layout.createLine()
   if not line.isValid():break
   line.setLineWidth(rect.width());line.setPosition(QPointF(0,y));y+=line.height()
   lines.append({"start":line.textStart(),"length":line.textLength(),"ascent":line.ascent(),"height":line.height(),"width":line.naturalTextWidth()})
  layout.endLayout();row["supplementary_qtextlayout"]={"lines":lines,"runs":[{"family":run.rawFont().familyName(),"glyphs":run.glyphIndexes(),"positions":[[v.x(),v.y()]for v in run.positions()]}for run in layout.glyphRuns()]};self.observed.append(row)
  return super().drawText(rect,flags,text)
cases=[("latin","Office AVATAR fi ffi",{}),("accents","Planta — sección ñ café",{}),("bold","Office AVATAR",{"bold":True}),("italic","Office AVATAR",{"italic":True}),("underline","Ground level",{"underline":True}),("wrap","Long project name with several words that wrap onto a second line",{"w_mm":32}),("newlines","Floor 1\nSection A",{}),("center","Centered title",{"align":"center"}),("right","Right title",{"align":"right"}),("serif","Office ffi AVATAR",{"family":"Serif"}),("mono","Office ffi AVATAR",{"family":"Monospace"}),("arabic","العربية 123",{}),("hebrew","שלום 123",{}),("cjk","平面図",{})]
if os.environ.get("INGETRAZO_TEXT_EXPANDED")=="1":
 cases += [("combining","A\u0301 e\u0301 q\u0308",{}),("kerns","To WA AV Yi",{}),("serif_bold","ff ffi fi fl",{"family":"Serif","bold":True}),("arabic_run","العربية",{}),("hebrew_run","שלום",{})]
if os.environ.get("INGETRAZO_TEXT_PARAGRAPHS")=="1":
 # Held-out source recipes: preserve whitespace, mixed fallback and paragraph boundaries.
 cases += [
  ("mixed_latin_cjk","Office 平面図 AVATAR",{}),
  ("mixed_cjk_rtl","平面図 العربية 123",{}),
  ("mixed_scripts","Latin العربية שלום 123",{}),
  ("spaces","  Floor   1  Section  A  ",{}),
  ("tabs","Floor\t1\tSection A",{}),
  ("nbsp","Floor\u00a01 Section\u00a0A",{"w_mm":22}),
  ("empty_paragraphs","Floor 1\n\nSection A\n",{}),
  ("long_word","ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz",{"w_mm":20}),
  ("mixed_wrapped","Office 平面図 several words العربية end",{"w_mm":32}),
  ("rtl_wrapped","שלום עולם العربية 123 repeated words",{"w_mm":32}),
  ("fallback_underline","Office 平面図",{"underline":True}),
  ("fallback_bold","Office 平面図",{"bold":True}),
 ]
if os.environ.get("INGETRAZO_TEXT_BACKGROUNDS")=="1":
 cases=[("bg_basic","Floor 1",{"bg_color":"#ffcc80"}),
 ("bg_wrapped","Long project name with several words that wrap onto a second line",{"w_mm":32,"bg_color":"#336699","bg_opacity":.35}),
 ("bg_empty","",{"bg_color":"#ffcc80"}),
 ("bg_cjk","Office 平面図",{"bg_color":"#ccff99"}),
 ("bg_rtl","שלום עולם العربية 123 repeated words",{"w_mm":32,"bg_color":"#cc99ff"}),
 ("bg_serif","Floor 1\nSection A",{"family":"Serif","bg_color":"#ffcc80"}),
 ("bg_bold","Floor 1\nSection A",{"bold":True,"bg_color":"#ffcc80"}),
 ("bg_italic","Floor 1",{"italic":True,"bg_color":"#ffcc80"}),
 ("bg_transparent","Floor 1",{"bg_color":"#ffcc80","bg_opacity":0})]
if os.environ.get("INGETRAZO_TEXT_LINEBREAKS")=="1":
 cases=[("punctuation","alpha/beta/gamma delta|epsilon|zeta 10/10/2026",{"w_mm":24}),
 ("cjk_wrap","平面図平面図平面図平面図",{"w_mm":18}),
 ("soft_hyphen","extra\u00adordinary extra\u00adordinary",{"w_mm":18}),
 ("line_separator","Floor 1\u2028Section A",{}),
 ("paragraph_separator","Floor 1\u2029Section A",{}),
 ("punctuation_rtl","שלום/עולם/שלום العربية/123",{"w_mm":24})]
if os.environ.get("INGETRAZO_TEXT_SHY_BOUNDARIES")=="1":
 cases=[("shy_"+str(w),"extra\u00adordinary extra\u00adordinary",{"w_mm":w})for w in [12,14,16,20,24,32]]
 cases += [("shy_prefix_"+str(w),"A extra\u00adordinary",{"w_mm":w})for w in [13,14,15,16]]
rows=[]
for name,text,props in cases:
 writer=QPdfWriter(str(out/(name+".original.pdf")));writer.setResolution(RENDER_DPI);writer.setPageSize(QPageSize(QSizeF(210,297),QPageSize.Unit.Millimeter));p=Observe(writer) if os.environ.get("INGETRAZO_TEXT_OBSERVER","1")=="1" else QPainter(writer);p.observed=[];p.scale(RENDER_DPI/25.4,RENDER_DPI/25.4);p.translate(20,30)
 item=TextoItem(text=text,size_pt=14,**props);paint_text_mm(p,item);p.end();row={"name":name,"authored_text":text,"properties":props,"source_draw_calls":p.observed}
 if props.get("bg_color"):
  fm,scale=_text_metrics(14*PT_TO_MM,props.get("bold",False),props.get("family","Sans Serif"));row["source_background"]={"ascent":fm.ascent(),"descent":fm.descent(),"cap_height":fm.capHeight(),"leading":fm.leading(),"line_spacing":fm.lineSpacing(),"line_count":_line_count(text,props.get("w_mm",80),14*PT_TO_MM,props.get("bold",False),props.get("family","Sans Serif")),"span_mm":text_bg_span_mm(text,props.get("w_mm",80),14*PT_TO_MM,props.get("bold",False),props.get("family","Sans Serif"))}
 rows.append(row)
(out/"font-observations.json").write_text(json.dumps({"source":"views.composer.paint_text_mm / _draw_text_mm","scope":"Actual source PDFs and font records; supplementary Qt layout is not a Rust implementation or parity proof","cases":rows},indent=2,ensure_ascii=False)+"\n")
print(f"PASS {len(rows)} actual original text painter PDFs and font observations; native text remains open")
