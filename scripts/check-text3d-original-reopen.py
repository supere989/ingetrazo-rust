#!/usr/bin/env python3
"""Reopen the native application consumer's IGZ in the pinned original reader."""
import json, sys
from pathlib import Path
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from core.text3d import text_is_pristine, letters_fingerprint
from formats.igz import load_into
app = QApplication([])
root = Path(sys.argv[1])
rows = []
for name, text, height, thickness in [("placed", "A O g", .25, .05), ("edited", "AV", .5, 20.), ("context-flat", "BO", .75, 0.)]:
    scene = Scene()
    load_into(scene, root / (name + ".igz"))
    assert len(scene.groups) == 1, name
    group = scene.groups[0]
    assert group.text3d["text"] == text and group.text3d["height"] == height and group.text3d["thickness"] == thickness, name
    assert [g.name for g in group.children] == [c for c in text if not c.isspace()], name
    assert not group.component and group.xform is not None, name
    assert all(g.xform is not None for g in group.children), name
    assert text_is_pristine(group), (name, group.text3d["hash"], letters_fingerprint(group.children))
    vertices = [v.position for child in group.children for v in child.mesh.vertices]
    assert abs(max(v.z() for v in vertices)-min(v.z() for v in vertices)-height)<1e-6, name
    assert abs(max(v.y() for v in vertices)-min(v.y() for v in vertices)-thickness)<1e-6, name
    rows.append({"name":name,"text":text,"letters":len(group.children),"pristine":True,"hash":group.text3d["hash"],"faces":sum(len(g.mesh.faces) for g in group.children),"placement":list(group.xform.data())})
boundary = Scene()
load_into(boundary, root / "fingerprint-boundary.igz")
assert text_is_pristine(boundary.groups[0]), (boundary.groups[0].text3d["hash"], letters_fingerprint(boundary.groups[0].children))
rows.append({"name":"fingerprint-boundary","pristine":True,"hash":boundary.groups[0].text3d["hash"]})
(root / "original-reopen.json").write_text(json.dumps({"source_pin":"6be29fe437a7cd992d4e079f71720821b25216b9","rows":rows},indent=2)+"\n")
print("Original reopened 3 native IGZ documents; text parameters, geometry and fingerprints match")
