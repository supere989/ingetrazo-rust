#!/usr/bin/env python3
"""Capture frozen original sheet schemas and stable visible paint order; no pixel claim."""
import dataclasses,json,sys
from pathlib import Path
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from core import composition as c
pairs=[("frames","MarcoVista"),("texts","TextoItem"),("images","ImagenItem"),("scalebars","BarraEscala"),("nortes","FlechaNorte"),("leyendas","Leyenda"),("shapes","FormaItem"),("cotas","CotaItem"),("cotas_ang","CotaAngularItem"),("cotas_rad","CotaRadialItem"),("etiquetas","EtiquetaItem"),("perfiles","PerfilTerreno"),("niveles","NivelItem"),("llamadas","LlamadaItem"),("cajetin","Cajetin")]
sheet=c.Composicion();rows=[];labels={}
for i,(key,name) in enumerate(pairs):
 item=getattr(c,name)();item.z=float(i%3);item.hidden=i in (2,8);labels[id(item)]=key
 if key=="cajetin":sheet.cajetin=item
 else:getattr(sheet,key).append(item)
 rows.append({"key":key,"class":name,"fields":[f.name for f in dataclasses.fields(item)],"defaults":dataclasses.asdict(item),"rust_native_painter":key in ("frames","texts","scalebars","nortes","cajetin")})
assert [labels[id(item)] for item in sheet.all_items()]==[key for key,_ in pairs]
order=[labels[id(item)]for item in sorted(sheet.all_items(),key=lambda item:item.z)if not item.hidden]
assert order==['frames', 'scalebars', 'shapes', 'cotas_rad', 'niveles', 'texts', 'nortes', 'cotas', 'etiquetas', 'llamadas', 'leyendas', 'perfiles', 'cajetin']
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
(out/"original-full-sheet.json").write_text(json.dumps(sheet.to_dict(),indent=2,ensure_ascii=False)+"\n")
(out/"schema-and-order.json").write_text(json.dumps({"original_revision":"6be29fe437a7cd992d4e079f71720821b25216b9","items":rows,"visible_paint_order":order,"scope":"Actual original dataclass schemas, serialization and stable z/hidden ordering. Native missing painters and PDF/image outputs remain open; no pixel acceptance."},indent=2,ensure_ascii=False)+"\n")
print("Captured15 actual original schemas;10 native painters missing")
