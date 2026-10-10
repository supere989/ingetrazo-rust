#!/usr/bin/env python3
"""Strict whole imported-model comparison; exact RGBA, no tolerance or crop."""
import sys, json, hashlib
from pathlib import Path
import numpy as np
from PySide6.QtGui import QImage

MANDATORY = ['gondola_v20-iso', 'gondola_v20-asymmetric', 'capilla_quiroz_v17-iso', 'capilla_quiroz_v17-asymmetric']

def read(path):
    image = QImage(str(path)).convertToFormat(QImage.Format_RGBA8888)
    if image.isNull() or (image.width(), image.height()) != (800, 600):
        raise ValueError('Missing or wrong-sized required image')
    return np.frombuffer(image.constBits(), dtype=np.uint8).reshape(600, image.bytesPerLine())[:, :3200].reshape(600, 800, 4).copy()

def compare(a, b):
    if a.shape != (600, 800, 4) or b.shape != a.shape:
        raise ValueError('Required RGBA extent differs')
    ink = int((a[:, :, :3].min(axis=2) < 245).sum())
    if ink <= 1000:
        raise ValueError('Original image lacks required ink')
    mask = np.any(a != b, axis=2)
    count = int(mask.sum())
    maximum = int(np.abs(a.astype(np.int16) - b.astype(np.int16)).max())
    coordinates = np.argwhere(mask)[:20]
    first = [[int(x), int(y), a[y, x].tolist(), b[y, x].tolist()] for y, x in coordinates]
    return dict(different_pixels=count, maximum_channel_delta=maximum, first_differences=first, source_ink_pixels=ink, exact=count == 0)

def main():
    reference, native, out = map(Path, sys.argv[1:4])
    out.mkdir(parents=True, exist_ok=True)
    protocol = json.loads((reference / 'protocol.json').read_text())
    assert protocol['source_pin'] == '6be29fe437a7cd992d4e079f71720821b25216b9'
    assert [r['name'] for r in protocol['cases']] == MANDATORY
    results = json.loads((native / 'native-world-results.json').read_text())
    assert [r['name'] for r in results['cases']] == MANDATORY and results['ledger_final'] == 0
    rows = []
    for name in MANDATORY:
        source_path = reference / (name + '-source.png')
        native_path = native / (name + '-native-vulkan.png')
        a, b = read(source_path), read(native_path)
        rows.append(dict(name=name, **compare(a, b), source_sha256=hashlib.sha256(source_path.read_bytes()).hexdigest(), native_sha256=hashlib.sha256(native_path.read_bytes()).hexdigest()))
    a = read(reference / (MANDATORY[0] + '-source.png'))
    assert compare(a, a)['exact']
    black = np.zeros_like(a); black[:, :, 3] = 255
    assert not compare(a, black)['exact']
    changed = a.copy(); changed[300, 400, 0] ^= 1
    assert compare(a, changed)['different_pixels'] == 1
    report = dict(passed=all(r['exact'] for r in rows), cases=rows, negatives=['black startup rejected', 'one altered pixel rejected'], scope='Strict complete native World images; same-device original reference required for hardware parity')
    (out / 'strict-results.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))
    return 0 if report['passed'] else 1

if __name__ == '__main__':
    raise SystemExit(main())
