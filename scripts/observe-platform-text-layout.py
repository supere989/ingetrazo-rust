#!/usr/bin/env python3
"""Observe the frozen text recipe through public Qt layout APIs without rendering."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import struct
import subprocess
import sys


def bits(value):
    return struct.unpack("<Q", struct.pack("<d", value))[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    assert not args.output.exists(), "Preserve first layout observation"
    repo = Path(__file__).resolve().parents[1]
    original = repo / "ingetrazo"
    pin = "6be29fe437a7cd992d4e079f71720821b25216b9"
    assert subprocess.check_output(["git", "-C", str(original), "rev-parse", "HEAD"], text=True).strip() == pin
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    sys.path.insert(0, str(original))
    from PySide6.QtCore import QPointF, qVersion
    from PySide6.QtGui import QFont, QFontMetricsF, QRawFont, QTextLayout, QTextOption, QTransform
    from PySide6.QtWidgets import QApplication
    from core.composition import TextoItem
    from views.composer import paint_text_mm
    assert qVersion() == "6.12.0"
    app = QApplication([])
    item = TextoItem(text="IngeTrazo\nPhysical sheet 1:100", x_mm=20, y_mm=30, w_mm=100)

    class Invocation:
        """Records the original Python painter arguments; does not emulate Qt's paint engine."""
        def __init__(self):
            self.transform = QTransform()
            self.transform.scale(100 / 25.4, 100 / 25.4)
            self.transform.translate(item.x_mm, item.y_mm)
            self.stack = []
            self.calls = []
        def save(self):
            self.stack.append(QTransform(self.transform))
        def restore(self):
            self.transform = self.stack.pop()
        def setFont(self, font):
            self.font = QFont(font)
        def setPen(self, pen):
            pass
        def scale(self, x, y):
            self.transform.scale(x, y)
        def drawText(self, rect, flags, text):
            assert len(self.calls) == 0
            self.calls.append((rect, flags, text, QFont(self.font), QTransform(self.transform)))

    observer = Invocation()
    paint_text_mm(observer, item)
    assert len(observer.calls) == 1 and not observer.stack
    rect, flags, text, font, transform = observer.calls[0]
    raw_font = QRawFont.fromFont(font)
    tables = {}
    for tag in ["head", "hhea", "maxp", "OS/2", "name", "cmap", "hmtx"]:
        data = bytes(raw_font.fontTable(tag))
        tables[tag] = {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
    # Keep each explicit source line at y=0. This directly observes shaping and
    # relative positions without pretending to reproduce private drawText line placement.
    rows = []
    for content in text.split("\n"):
        layout = QTextLayout(content, font)
        option = QTextOption()
        option.setWrapMode(QTextOption.NoWrap)
        layout.setTextOption(option)
        layout.beginLayout()
        line = layout.createLine()
        assert line.isValid()
        line.setLineWidth(rect.width())
        line.setPosition(QPointF(0, 0))
        assert not layout.createLine().isValid()
        layout.endLayout()
        runs = []
        for run in line.glyphRuns():
            positions = [[p.x(), p.y()] for p in run.positions()]
            runs.append({"glyphs": list(run.glyphIndexes()), "positions": positions,
                         "position_bits": [[bits(v) for v in point] for point in positions],
                         "font_family": run.rawFont().familyName(), "font_style": run.rawFont().styleName()})
        rows.append({"text": content, "ascent": line.ascent(), "descent": line.descent(),
                     "leading": line.leading(), "height": line.height(), "advance": line.horizontalAdvance(), "runs": runs})
    m = [transform.m11(), transform.m12(), transform.m21(), transform.m22(), transform.dx(), transform.dy()]
    result = {"source_pin": pin, "qt": qVersion(), "machine": platform.machine(),
              "source_files": {name: hashlib.sha256((original / name).read_bytes()).hexdigest()
                               for name in ["views/composer.py", "core/composition.py"]},
              "observer_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "font_tables": tables, "lines": rows, "dpi": 100,
              "paint_call": {"text": text, "rect": [rect.x(), rect.y(), rect.width(), rect.height()],
                             "flags": flags, "pixel_size": font.pixelSize(), "family": font.family(),
                             "matrix": m, "matrix_bits": [bits(v) for v in m]},
              "fresh_images": 0,
              "scope": "Actual original Python paint_text_mm call arguments and independent public QTextLayout shaping of each explicit line at y=0. No actual QPainter engine, private device QFixed positions, line placement or cached phase count is observed."}
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"lines": len(rows), "glyphs": sum(len(r["glyphs"]) for row in rows for r in row["runs"]), "fresh_images": 0}))


if __name__ == "__main__":
    main()
