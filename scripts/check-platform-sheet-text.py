#!/usr/bin/env python3
"""One actual native-platform original sheet-text capture and prebuilt GPU replay."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
from types import SimpleNamespace


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("worker", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    assert sys.platform == "darwin", "This missing oracle is specifically native macOS text"
    repo = Path(__file__).resolve().parents[1]
    binary, worker, out = args.binary.resolve(), args.worker.resolve(), args.output.resolve()
    assert not out.exists(), "Preserve the first platform observation in a fresh directory"
    pin = "6be29fe437a7cd992d4e079f71720821b25216b9"
    assert subprocess.check_output(["git", "-C", str(repo / "ingetrazo"), "rev-parse", "HEAD"], text=True).strip() == pin
    identities = {"consumer": sha(binary), "worker": sha(worker)}
    out.mkdir(parents=True)
    source, native = out / "source", out / "native"
    source.mkdir()
    os.environ["QT_QPA_PLATFORM"] = "offscreen"
    os.environ["XDG_CONFIG_HOME"] = str(out / "config")
    os.environ["XDG_DATA_HOME"] = str(out / "data")
    os.environ["XDG_CACHE_HOME"] = str(out / "cache")
    os.environ["INGETRAZO_STATE_DIR"] = str(out / "state")
    sys.path.insert(0, str(repo / "ingetrazo"))
    from PySide6.QtCore import qVersion
    from PySide6.QtGui import QFont, QFontInfo, QFontMetricsF, QImage, QRawFont
    from PySide6.QtWidgets import QApplication, QMainWindow
    from core.composition import Composicion, TextoItem
    from core.scene import Scene
    from views.composer import ComposerWindow

    app = QApplication([])
    app.setOrganizationName("IngeTrazoPlatformTextParity")
    app.setApplicationName("Source")

    class Owner(QMainWindow):
        IMAGE_EXPORT_DPI = ComposerWindow.IMAGE_EXPORT_DPI
        _paint_sheet = ComposerWindow._paint_sheet
        _set_field_context = ComposerWindow._set_field_context
        export_image = ComposerWindow.export_image

        def __init__(self):
            super().__init__()
            self.comp = Composicion(paper="A4", landscape=False)
            self.comp.texts = [TextoItem(text="IngeTrazo\nPhysical sheet 1:100", x_mm=20, y_mm=30, w_mm=100)]
            self.scene = Scene()
            self.scene.compositions = [self.comp]
            self._window = SimpleNamespace(_current_path=None)

        def _scene(self):
            return self.scene

    owner = Owner()
    owner.export_image(str(source / "text.png"), 100)
    image = QImage(str(source / "text.png")).convertToFormat(QImage.Format_RGB888)
    assert not image.isNull()
    raw = bytes(image.constBits())
    rgb = b"".join(raw[y * image.bytesPerLine():y * image.bytesPerLine() + image.width() * 3] for y in range(image.height()))
    font = QFont("Sans Serif")
    font.setPixelSize(100)
    info, metrics, raw_font = QFontInfo(font), QFontMetricsF(font), QRawFont.fromFont(font)
    assert raw_font.isValid()
    tables = {}
    for name in ["head", "hhea", "maxp", "OS/2", "name", "cmap", "hmtx", "glyf", "loca", "CFF ", "CFF2"]:
        data = bytes(raw_font.fontTable(name))
        if data:
            tables[name] = {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
    imported = {}
    original = (repo / "ingetrazo").resolve()
    for module in list(sys.modules.values()):
        file = getattr(module, "__file__", None)
        if not file:
            continue
        path = Path(file).resolve()
        if path.is_relative_to(original) and path.is_file():
            imported[path.relative_to(original).as_posix()] = sha(path)
    row = {"case": "text", "paper": "A4", "landscape": False, "dpi": 100, "content": "text",
           "image": {"file": "text.png", "size": [image.width(), image.height()], "rgb_sha256": hashlib.sha256(rgb).hexdigest()}}
    (source / "source.json").write_text(json.dumps({"source_pin": pin, "images": [row]}, indent=2) + "\n")
    receipt = {"source_pin": pin, "source_files": imported, "platform": platform.platform(), "machine": platform.machine(),
               "qt": qVersion(), "font": {"requested_family": "Sans Serif", "pixel_size": 100,
               "family": info.family(), "style": info.styleName(), "raw_family": raw_font.familyName(),
               "raw_style": raw_font.styleName(), "ascent": metrics.ascent(), "descent": metrics.descent(),
               "height": metrics.height(), "tables": tables}, "image_sha256": sha(source / "text.png"),
               "rgb_sha256": row["image"]["rgb_sha256"], "fresh_source_images": 1,
               "scope": "Original export_image/_paint_sheet on this macOS runner. One platform text recipe; no desktop/font inventory claim."}
    (source / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    owner.close()
    command = [str(binary), str(source), str(native), "--worker", str(worker)]
    with (out / "runtime.log").open("w") as stdout, (out / "runtime.stderr").open("w") as stderr:
        completed = subprocess.run(command, cwd=repo, env=os.environ.copy(), stdout=stdout, stderr=stderr, timeout=120)
    if completed.returncode:
        (out / "result.json").write_text(json.dumps({"result": "runtime-failure", "returncode": completed.returncode, "binaries": identities, "command": command}, indent=2) + "\n")
        raise SystemExit(completed.returncode)
    record = json.loads((native / "native.json").read_text())
    assert [v["case"] for v in record["images"]] == ["text"]
    observed = record["images"][0]
    assert observed["file"] == "text.png" and observed["pixels"] == row["image"]["size"]
    assert observed["dpi_remembered"] == observed["metrics"]["dpi"] == 100
    spec = importlib.util.spec_from_file_location("sheet_image_check", repo / "scripts/check-sheet-image-native.py")
    check = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(check)
    a = check.Image.open(source / "text.png").tobytes()
    b = check.Image.open(native / "text.png").tobytes()
    assert len(a) == len(b)
    assert hashlib.sha256(a).hexdigest() == row["image"]["rgb_sha256"]
    different = sum(a[i:i+3] != b[i:i+3] for i in range(0, len(a), 3))
    maximum = max((abs(x-y) for x, y in zip(a, b)), default=0)
    metadata_exact = check.physical(source / "text.png") == check.physical(native / "text.png")
    result = {"result": "pass" if different == 0 and metadata_exact else "mismatch", "binaries": identities,
              "source": receipt, "command": command, "different_pixels": different, "max_channel_error": maximum,
              "physical_headers_exact": metadata_exact, "native_rgb_sha256": hashlib.sha256(b).hexdigest(),
              "scope": "Strict whole-image original/native macOS text comparison. A mismatch remains a failure; geometry/codec evidence is separate."}
    (out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({k: v for k, v in result.items() if k not in {"source", "command"}}))
    raise SystemExit(0 if result["result"] == "pass" else 1)


if __name__ == "__main__":
    main()
