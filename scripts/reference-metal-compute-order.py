#!/usr/bin/env python3
"""Actual Qt ordered opaque UI / authored CAD / opaque UI reference."""
import hashlib,json,pathlib,sys
from PySide6.QtGui import QImage,QPainter,QColor
import PySide6
root=pathlib.Path(sys.argv[1]);root.mkdir(parents=True,exist_ok=True)
if any(root.iterdir()):raise ValueError("fresh source output required")
image=QImage(64,64,QImage.Format_RGBA8888);image.fill(QColor(255,255,255))
painter=QPainter(image)
painter.fillRect(0,0,64,64,QColor(0,0,255))
painter.fillRect(8,8,48,48,QColor(37,93,157,128))
painter.fillRect(16,16,32,32,QColor(1,7,19,128))
painter.fillRect(32,32,8,8,QColor(0,255,0))
painter.end();data=bytes(image.constBits());assert len(data)==64*64*4
(root/"mixed-ui-cad.rgba").write_bytes(data)
(root/"source.json").write_text(json.dumps({"qt":PySide6.__version__,"format":"RGBA8888","width":64,"height":64,"sha256":hashlib.sha256(data).hexdigest(),"actual_QPainter":True,"ordered_fills":["opaque_blue_UI","authored_outer_alpha128","authored_inner_alpha128","opaque_green_UI"]},indent=2)+"\n")
