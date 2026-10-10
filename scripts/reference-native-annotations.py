#!/usr/bin/env python3
"""Execute pinned source annotation methods with controlled projection/visibility.
This is a geometry/style oracle, not actual viewport or GPU occlusion acceptance.
"""
import ast, hashlib, json, math, os, subprocess, sys
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
from PySide6.QtCore import Qt, QPointF
from PySide6.QtGui import QColor, QFont, QImage, QPainter, QPen, QPolygonF, QVector3D
from PySide6.QtWidgets import QApplication
source, output = map(Path, sys.argv[1:3])
assert subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip() == "6be29fe437a7cd992d4e079f71720821b25216b9"
sys.path.insert(0, str(source))
from core.dimension import Dimension
from core.textlabel import TextLabel
app = QApplication.instance() or QApplication([])
path = source / "views/viewport.py"
tree = ast.parse(path.read_text())
names = {"_text_block_x", "_draw_text_labels", "_draw_dimensions", "_format_dim_value", "_draw_occluded_segment"}
methods = [n for cls in tree.body if isinstance(cls, ast.ClassDef) for n in cls.body if isinstance(n, ast.FunctionDef) and n.name in names]
assert {n.name for n in methods} == names
module = ast.Module(body=[ast.ClassDef(name="SourceMethods", bases=[], keywords=[], body=methods, decorator_list=[])], type_ignores=[])
namespace = dict(globals())
exec(compile(ast.fix_missing_locations(module), str(path), "exec"), namespace)
SourceMethods = namespace["SourceMethods"]
def point(p): return [p.x(), p.y()]
class Painter(QPainter):
    def __init__(self, image):
        super().__init__(image); self.ops=[]
    def entry(self, kind, **data):
        self.ops.append(dict(kind=kind, pen=list(self.pen().color().getRgb()), width=self.pen().widthF(), pen_style=self.pen().style().value, **data))
    def drawLine(self, a, b):
        self.entry("line", a=point(a), b=point(b)); super().drawLine(a,b)
    def drawPolygon(self, p):
        self.entry("polygon", points=[point(x) for x in p], brush=list(self.brush().color().getRgb())); super().drawPolygon(p)
    def drawEllipse(self, p, rx, ry):
        self.entry("ellipse", center=point(p), radius=[rx,ry], brush=list(self.brush().color().getRgb())); super().drawEllipse(p,rx,ry)
    def drawText(self, p, text):
        font=self.font(); self.entry("text", position=point(p), text=text, font=[font.family(),font.pointSizeF(),font.bold()]); super().drawText(p,text)
class View(SourceMethods):
    def _world_to_pixel(self,p):
        if self.reject_anchor and p.x()<0: return None
        return (160.+p.x()*30.,160.-p.y()*30.)
    def _occluded_on_screen(self,p):
        return self.occluded and 1.25 <= p.x() <= 2.75
output.mkdir(parents=True,exist_ok=True)
rows=[]
for name,kind,ends,selected,hidden,occluded,reject in [
    ("arrow","dim","arrow",False,False,False,False),
    ("tick-selected","dim","tick",True,False,False,False),
    ("none","dim","none",False,False,False,False),
    ("partial-occlusion","dim","arrow",False,False,True,False),
    ("hidden","dim","arrow",False,True,False,False),
    ("leader-right","leader","arrow",False,False,False,False),
    ("leader-left-selected","left","arrow",True,False,False,False),
    ("leader-anchor-unprojectable","leader","arrow",False,False,False,True),
    ("leader-hidden","leader","arrow",False,True,False,False),
    ("leader-empty","empty","arrow",False,False,False,False),
    ("dim-hover","dim","arrow",False,False,False,False),
    ("dim-mm-axis","dim","tick",False,False,False,False),
    ("dim-feet-fractional","dim","none",False,False,False,False),
    ("leader-hover","leader","arrow",False,False,False,False),
    ("leader-splitlines","leader","arrow",False,False,False,False),
]:
    entity = Dimension(QVector3D(0,0,0),QVector3D(4,0,0),QVector3D(0,2,0),text="Length <> ") if kind=="dim" else TextLabel(QVector3D(-1 if reject else 4,0,0),QVector3D(-3 if kind=="left" else 3,2,0),"" if kind=="empty" else "First line\nSecond line")
    if name=="dim-mm-axis":
        entity.b=QVector3D(3,4,0);entity.axis="x"
    if name=="leader-splitlines": entity.text="A\r\nB\u2028C\n"
    view=View();view.reject_anchor=reject;view.occluded=occluded;view._hover_entity=entity if name.endswith("hover") else None
    view.scene=SimpleNamespace(dimensions=[entity] if kind=="dim" else [],text_labels=[] if kind=="dim" else [entity],dimension_style={"color":[12,65,91],"font_size":11,"ends":ends,"units":"cm","decimals":1},selection=[entity] if selected else [],entity_visible=lambda e:not hidden)
    if name=="dim-mm-axis": view.scene.dimension_style.update(units="mm",decimals=0)
    if name=="dim-feet-fractional": view.scene.dimension_style.update(units="ft-in-frac",decimals=2)
    image=QImage(640,400,QImage.Format_RGBA8888);image.fill(QColor("white"));painter=Painter(image);painter.setRenderHint(QPainter.Antialiasing)
    view._draw_dimensions(painter);view._draw_text_labels(painter);ops=painter.ops;painter.end()
    assert image.save(str(output/(name+".png")))
    rows.append(dict(name=name,style=view.scene.dimension_style,entity=({"a":entity.a.toTuple(),"b":entity.b.toTuple(),"offset":entity.offset.toTuple(),"axis":entity.axis,"text":entity.text} if kind=="dim" else entity.to_dict()),selected=selected,hovered=view._hover_entity is entity,hidden=hidden,controlled_occlusion=occluded,reject_negative_x_projection=reject,operations=ops,image_sha256=hashlib.sha256((output/(name+".png")).read_bytes()).hexdigest()))
assert len(rows[0]["operations"])==52
assert rows[4]["operations"]==[] and rows[8]["operations"]==[]
assert not any(o["kind"]=="ellipse" for o in rows[7]["operations"])
assert any(o["kind"]=="text" for o in rows[7]["operations"])
assert len(rows[3]["operations"]) < len(rows[0]["operations"])
assert all(o["pen"][:3]==[243,115,41] for o in rows[10]["operations"] if o["kind"]=="line")
assert any(o["kind"]=="text" and "3000 mm" in o["text"] for o in rows[11]["operations"])
assert any(o["kind"]=="text" and "'" in o["text"] for o in rows[12]["operations"])
assert [o["text"] for o in rows[14]["operations"] if o["kind"]=="text"]==["A","A","B","B","C","C"]
(output/"original.json").write_text(json.dumps(dict(source_pin="6be29fe437a7cd992d4e079f71720821b25216b9",viewport_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),scope="Actual source annotation methods and entities; controlled projection/occlusion, not GPU or interactive viewport acceptance",cases=rows),indent=2)+"\n")
print(json.dumps({"cases":len(rows),"operations":[len(r["operations"]) for r in rows]}))
