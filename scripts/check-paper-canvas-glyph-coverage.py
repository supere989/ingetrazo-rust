#!/usr/bin/env python3
"""Actual original QRawFont alpha maps versus shared native FT before compositing.
Phase-zero controls isolate raster coverage; full cached TextItem canvas pixels remain separate.
"""
import os,json,sys,hashlib
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
from PySide6.QtGui import QGuiApplication,QFont,QRawFont,QTransform
from PySide6.QtCore import qVersion
app=QGuiApplication([]);font=QFont("Sans Serif");font.setPixelSize(100);raw=QRawFont.fromFont(font)
native=json.loads(Path(sys.argv[1]).read_text());rows=[]
for row in native["rows"]:
 if row["phase"]!=0:continue
 glyph=raw.glyphIndexesForString(row["text"])[0];assert glyph==row["glyph"]
 image=raw.alphaMapForGlyph(glyph,QRawFont.PixelAntialiasing,QTransform.fromScale(row["scale"],row["scale"]))
 coverage=[image.pixelColor(x,y).alpha()for y in range(image.height())for x in range(image.width())]
 assert [image.width(),image.height()]==[row["width"],row["height"]]
 assert coverage==row["coverage"],"Actual FT glyph coverage mismatch"
 altered=row["coverage"][:];altered[len(altered)//2]^=1;assert altered!=coverage,"Single coverage-bit negative failed"
 rows.append({"text":row["text"],"glyph":glyph,"scale":row["scale"],"width":image.width(),"height":image.height(),"coverage":coverage})
assert len(rows)==4
report={"passed":True,"rows":rows,"qt_version":qVersion(),"native_font_path":native["font_path"],"native_data_sha256":hashlib.sha256(Path(sys.argv[1]).read_bytes()).hexdigest(),"checker_sha256":hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),"scope":__doc__}
Path(sys.argv[2]).write_text(json.dumps(report,indent=2)+"\n");print("Four actual original/native phase-zero glyph maps exact; one-bit negatives rejected; full canvas remains failing")
