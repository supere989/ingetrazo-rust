#!/usr/bin/env python3
"""Replay integrated app consumers against authenticated, unchanged source inputs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import zipfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("binaries", type=Path, help="prebuilt example directory")
parser.add_argument("output", type=Path)
args = parser.parse_args()
repo = Path(__file__).resolve().parents[1]
binaries = args.binaries.resolve()
out = args.output.resolve()
out.mkdir(parents=True, exist_ok=True)
pin = "6be29fe437a7cd992d4e079f71720821b25216b9"
assert subprocess.check_output(["git", "-C", str(repo/"ingetrazo"), "rev-parse", "HEAD"], text=True).strip() == pin
suffix = ".exe" if os.name == "nt" else ""
consumers = ["change_axes_fixture", "document_preservation_workflow", "pushpull_solid_workflows", "fillet_scoped", "pushpull_finite_workflows", "paste_save_workflow", "box_selection_fixture", "material_restamp_workflow", "material_tray_egui", "cursor_navigation_workflow", "annotation_delete_fixture", "zoom_extents_workflow", "annotation_move_fixture", "annotation_acquisition_fixture", "mesh_kernel_acceptance", "annotation_edit_style_fixture", "annotation_text_create_fixture", "mesh_finite_kernel_fixture", "annotation_hover_fixture", "dimension_anchor_acceptance_fixture"]
identities = {name: hashlib.sha256((binaries/(name+suffix)).read_bytes()).hexdigest() for name in consumers}
env = dict(os.environ, QT_QPA_PLATFORM="offscreen", INGETRAZO_STATE_DIR=str(out/"state"),
           XDG_CONFIG_HOME=str(out/"config"), XDG_DATA_HOME=str(out/"data"), XDG_CACHE_HOME=str(out/"cache"))

def archive(name, manifest_name, selected, destination, preserve_paths=False, archive_name="records.zip"):
    folder = repo/"docs/audits"/name
    manifest = json.loads((folder/manifest_name).read_text())
    records = manifest.get("records", manifest.get("entries", manifest.get("members")))
    assert records is not None, name
    if isinstance(records, list):
        rows = records
        records = {entry["path"]: entry for entry in rows}
        assert len(records) == len(rows), "Duplicate archive member: " + name
    raw = (folder/archive_name).read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    expected_archive = manifest.get("archive_sha256", manifest.get("records_sha256"))
    if expected_archive is not None:
        assert digest == expected_archive, name
    destination.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(folder/archive_name) as z:
        assert set(z.namelist()) == set(records), name
        for key, expected in records.items():
            blob = z.read(key)
            digest = expected if isinstance(expected, str) else expected["sha256"]
            assert hashlib.sha256(blob).hexdigest() == digest, key
            if isinstance(expected, dict):
                assert len(blob) == expected["bytes"], key
            if key in selected:
                target = destination/(key if preserve_paths else Path(key).name)
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(blob)
        assert set(selected) <= set(z.namelist()), name
    return hashlib.sha256(raw).hexdigest()

def run(argv, output):
    with (out/output).open("w") as stdout, (out/(output+".stderr")).open("w") as stderr:
        subprocess.run([str(v) for v in argv], cwd=repo, env=env, stdout=stdout, stderr=stderr, check=True, timeout=120)

axes = out/"axes"
axis_cases = ["classic", "shared", "nested", "oblique", "context"]
a = archive("change-axes", "provenance.json", [c+".input.igz" for c in axis_cases]+["original.json"], axes)
doc = out/"documents"
d = archive("document-preservation-workflow", "manifest.json", ["capture/retained.igz", "capture/nested-leaves.igz", "capture/expected.json"], doc)
run([binaries/(consumers[0]+suffix), axes], "axes/native.json")
run([sys.executable, repo/"scripts/reference-change-axes.py", axes, "inspect"], "axes/reopened.json")
run([sys.executable, repo/"scripts/check-change-axes.py", axes], "axes/check.json")
run([binaries/(consumers[1]+suffix), doc], "documents/native.json")
run([sys.executable, repo/"scripts/reference-document-preservation-workflow.py", "compare", doc], "documents/compare.json")
run([sys.executable, repo/"scripts/check-document-preservation-workflow.py", doc/"observed.json"], "documents/check.log")
fillet = out/"fillet"
f = archive("fillet-nested-history", "provenance.json", ["source.json", "source-app.json", "old-native.json"], fillet)
run([binaries/(consumers[3]+suffix), fillet/"source.json", fillet/"native.json"], "fillet/runtime.log")
run([sys.executable, repo/"scripts/check-fillet-nested-history.py", fillet], "fillet/check.json")
run([sys.executable, repo/"scripts/check-fillet-independent-volume.py", repo/"docs/audits", fillet/"volume.json", fillet/"native.json"], "fillet/volume.log")
run([binaries/(consumers[2]+suffix), repo/"docs/audits/pushpull-solid-workflows/app-input.json", out/"pushpull-app.json"], "pushpull-runtime.log")
run([sys.executable, repo/"scripts/check-pushpull-solid-workflows.py", repo/"docs/audits/pushpull-solid-workflows", out/"pushpull-comparison.json", out/"pushpull-app.json"], "pushpull-check.log")
finite = out/"finite"
p = archive("pushpull-finite-corpus", "provenance.json", ["source.json"], finite)
run([binaries/(consumers[4]+suffix), finite/"source.json", finite/"native.json"], "finite/runtime.log")
run([sys.executable, repo/"scripts/check-pushpull-finite-corpus.py", repo/"docs/audits/pushpull-finite-corpus", finite/"comparison.json", finite/"native.json"], "finite/check.log")
paste = out/"paste-save"
ps = archive("paste-save", "provenance.json", ["input.igz", "original.json"], paste)
run([binaries/(consumers[5]+suffix), paste], "paste-save/runtime.log")
run([sys.executable, repo/"scripts/reference-paste-save.py", paste, "inspect"], "paste-save/reopen.log")
run([sys.executable, repo/"scripts/check-paste-save.py", paste], "paste-save/acceptance.json")
box = out/"box-selection"
bs = archive("box-selection", "provenance.json", ["input.igz", "original.json", "orphan-input.igz", "guide-input.igz", "failures/missing-guides-native.json", "failures/face-layer-gate-native.json"], box, preserve_paths=True)
run([binaries/(consumers[6]+suffix), box], "box-selection/runtime.log")
run([sys.executable, repo/"scripts/reference-box-guides.py", "reopen", box], "box-selection/reopen.log")
run([sys.executable, repo/"scripts/check-box-selection.py", box], "box-selection/acceptance.json")
material = out/"material-edit-restamp"
mr = archive("material-edit-restamp", "provenance.json", [], material)
run([binaries/(consumers[7]+suffix), material], "material-edit-restamp/runtime.log")
run([binaries/(consumers[8]+suffix), material], "material-edit-restamp/tray-runtime.log")
run([sys.executable, repo/"scripts/reopen-material-restamp.py", material, material/"reopened.json"], "material-edit-restamp/reopen.log")
run([sys.executable, repo/"scripts/check-material-restamp.py", "--native", material/"native.json", "--tray", material/"tray.json", "--reopened", material/"reopened.json"], "material-edit-restamp/acceptance.json")
cursor = out/"cursor-pivot-navigation"
cn = archive("cursor-pivot-navigation", "manifest.json", ["original.json", "old-dispatch-native.json"], cursor)
run([binaries/(consumers[9]+suffix), cursor/"original.json", cursor/"native.json"], "cursor-pivot-navigation/runtime.log")
run([sys.executable, repo/"scripts/check-cursor-navigation.py", cursor/"original.json", cursor/"native.json"], "cursor-pivot-navigation/acceptance.json")
with (cursor/"old-dispatch-rejection.log").open("w") as failure:
    rejected = subprocess.run([sys.executable, str(repo/"scripts/check-cursor-navigation.py"), str(cursor/"original.json"), str(cursor/"old-dispatch-native.json")], cwd=repo, env=env, stdout=failure, stderr=failure, timeout=120)
assert rejected.returncode != 0, "Compiled original cursor dispatcher unexpectedly passed"
annotations = out/"annotation-delete"
ad = archive("annotation-delete", "manifest.json", ["delete-input.igz", "original.json"]+[name+"-original.igz" for name in ["root", "nested-annotations", "nested-mixed", "parked"]], annotations)
ahd = archive("annotation-hover-delete", "manifest.json", ["hover-input.igz", "hover-delete-original.json"], annotations)
run([binaries/(consumers[10]+suffix), annotations], "annotation-delete/runtime.log")
run([sys.executable, repo/"scripts/check-annotation-delete.py", annotations], "annotation-delete/acceptance.json")
run([sys.executable, repo/"scripts/check-annotation-delete-reopen.py", annotations], "annotation-delete/reopen-acceptance.json")
zoom = out/"zoom-extents"
zoom_cases = ["parallel", "perspective", "two-point", "portrait", "hidden-nested", "empty", "orphan", "figure-only", "survey-only", "survey-hidden", "hidden-wire", "hidden-face"]
ze = archive("zoom-extents", "provenance.json", ["capture/source.json"]+["capture/"+case+".igz" for case in zoom_cases], zoom)
run([binaries/(consumers[11]+suffix), zoom], "zoom-extents/runtime.log")
run([sys.executable, repo/"scripts/check-zoom-extents.py", "--native", zoom/"native.json"], "zoom-extents/acceptance.json")
move = out/"annotation-move"
am = archive("annotation-move", "manifest.json", ["input.igz", "move-input.igz", "one-riding-input.igz", "original.json", "annotation-move-original-first-failure.json", "annotation-move-native-first-failure.json"], move)
run([binaries/(consumers[12]+suffix), move], "annotation-move/runtime.log")
run([sys.executable, repo/"scripts/check-annotation-move.py", move], "annotation-move/acceptance.json")
acquisition = out/"annotation-acquisition"
aa = archive("annotation-acquisition/riding-order", "manifest.json", ["input.igz", "riding-input.igz", "original.json", "threshold-first-failure.json", "source-first-failure.log"], acquisition, archive_name="source-records.zip")
run([binaries/(consumers[13]+suffix), acquisition], "annotation-acquisition/runtime.log")
run([sys.executable, repo/"scripts/check-annotation-acquisition.py", acquisition], "annotation-acquisition/acceptance.json")
mesh = out/"mesh-kernel"
mk = archive("p05-mesh-kernel", "manifest.json", ["input.json", "source.json"], mesh)
run([binaries/(consumers[14]+suffix), mesh/"input.json"], "mesh-kernel/native.json")
run([sys.executable, repo/"scripts/check-mesh-kernel.py", mesh/"source.json", mesh/"native.json"], "mesh-kernel/acceptance.json")
annotation_edit = out/"annotation-edit-style"
ae = archive("annotation-edit-style", "manifest.json", ["input.igz", "original.json", "failed-nested-style.json", "source-label-priority.json"] + [kind+"-"+context+".igz" for kind in ["label", "dimension"] for context in ["root", "nested"]], annotation_edit)
run([binaries/(consumers[15]+suffix), annotation_edit], "annotation-edit-style/runtime.log")
run([sys.executable, repo/"scripts/reopen-annotation-edit-style.py", annotation_edit], "annotation-edit-style/reopen.log")
run([sys.executable, repo/"scripts/check-annotation-edit-style.py", annotation_edit], "annotation-edit-style/acceptance.json")
annotation_text = out/"annotation-text-create"
text_cases = [context+"-"+kind for context in ["root", "nested"] for kind in ["edge", "face", "coordinates"]]+["closed-face"]
at = archive("annotation-text-create", "manifest.json", ["input.igz", "original.json", "original-history.json", "failed-unknown-label-root-edge.igz", "failed-group-edge-suggestion.json"]+[name+ending for name in text_cases for ending in ["-input.igz", ".igz"]], annotation_text)
run([binaries/(consumers[16]+suffix), annotation_text], "annotation-text-create/runtime.log")
run([sys.executable, repo/"scripts/reopen-annotation-text-create.py", annotation_text], "annotation-text-create/reopen.log")
run([sys.executable, repo/"scripts/check-annotation-text-create.py", annotation_text], "annotation-text-create/acceptance.jsonl")
finite_mesh = out/"finite-kernel"
finite_names = ["empty", "one", "two", "repeated", "welded-close", "empty-hole", "one-hole", "two-hole", "empty-outer-proper-hole"]
mf = archive("p05-finite-kernel", "manifest.json", ["original.json"]+[name+".original.igz" for name in finite_names], finite_mesh)
mfa = archive("p05-finite-acceptance", "manifest.json", ["initial-native.json"], finite_mesh)
run([binaries/(consumers[17]+suffix), finite_mesh], "finite-kernel/runtime.log")
run([sys.executable, repo/"scripts/reopen-p05-finite-kernel.py", finite_mesh], "finite-kernel/reopen.log")
run([sys.executable, repo/"scripts/check-p05-finite-invariants.py", finite_mesh, "--initial", finite_mesh/"initial-native.json", "--initial-sha256", "519bc52b68587c56ed3c4f5683d7945ad9de24bd0e862c808b716b2eb9237aa9", "--report", finite_mesh/"acceptance.json"], "finite-kernel/checker.log")
annotation_hover = out/"annotation-hover"
ah = archive("annotation-hover", "manifest.json", ["input.igz", "original-hover.json", "first40-native-weak.json"], annotation_hover)
run([binaries/(consumers[18]+suffix), annotation_hover], "annotation-hover/runtime.log")
run([sys.executable, repo/"scripts/check-annotation-hover.py", annotation_hover], "annotation-hover/acceptance.json")
dimension_anchors = out/"dimension-anchor-admission"
anchor_cases = ["near", "outside", "nested-shared", "billboard", "orphan"]
da = archive("dimension-anchor-admission", "manifest.json", [name+ending for name in anchor_cases for ending in ["-input.igz", "-original.igz"]]+["admission-original.json", "first-native-inverse-mismatch.json"], dimension_anchors)
run([binaries/(consumers[19]+suffix), dimension_anchors, "--admission"], "dimension-anchor-admission/runtime.log")
run([sys.executable, repo/"scripts/check-dimension-anchor-admission.py", dimension_anchors], "dimension-anchor-admission/acceptance.json")
run([sys.executable, repo/"scripts/reopen-dimension-anchor-admission.py", dimension_anchors], "dimension-anchor-admission/reopen-acceptance.json")
rows = json.loads((doc/"native.json").read_text())["rows"]
assert [(x["fixture"],x["phase"]) for x in rows] == [(f,p) for f in ["retained","nested-leaves"] for p in ["edited","undo","redo"]]
assert all(x["saved"] and x["clean"] for x in rows)
result = {"revision":subprocess.check_output(["git","rev-parse","HEAD"],cwd=repo,text=True).strip(),
          "source_pin":pin,"binary_sha256":identities,"source_archives":{"axes":a,"documents":d,"fillet":f,"pushpull_finite":p,"paste_save":ps,"box_selection":bs,"materials":mr,"cursor":cn,"annotations":ad,"annotation_hover_delete":ahd,"zoom_extents":ze,"annotation_move":am,"annotation_acquisition":aa,"mesh_kernel":mk,"annotation_edit_style":ae,"annotation_text_create":at,"finite_kernel":mf,"finite_kernel_initial":mfa,"annotation_hover":ah,"dimension_anchor_admission":da},
          "axes":json.loads((axes/"check.json").read_text()),"documents":{"actual_app_saves":len(rows),"exact_original_reopens":6,"leaf_record_types":15,"negative_controls":5},
          "fillet":json.loads((fillet/"check.json").read_text()),"fillet_volume":json.loads((fillet/"volume.json").read_text()),"pushpull":json.loads((out/"pushpull-comparison.json").read_text()),
          "pushpull_finite":json.loads((finite/"comparison.json").read_text()),
          "paste_save":json.loads((paste/"acceptance.json").read_text()),
          "box_selection":json.loads((box/"acceptance.json").read_text()),
          "materials":json.loads((material/"acceptance.json").read_text()),
          "cursor":json.loads((cursor/"acceptance.json").read_text()),
          "annotations":json.loads((annotations/"acceptance.json").read_text()),
          "annotation_reopens":json.loads((annotations/"reopen-acceptance.json").read_text()),
          "zoom_extents":json.loads((zoom/"acceptance.json").read_text()),
          "annotation_move":json.loads((move/"acceptance.json").read_text()),
          "annotation_acquisition":json.loads((acquisition/"acceptance.json").read_text()),
          "mesh_kernel":json.loads((mesh/"acceptance.json").read_text()),
          "annotation_edit_style":json.loads((annotation_edit/"acceptance.json").read_text()),
          "annotation_text_create":[json.loads(line) for line in (annotation_text/"acceptance.jsonl").read_text().splitlines()],
          "finite_kernel":json.loads((finite_mesh/"acceptance.json").read_text()),
          "annotation_hover":json.loads((annotation_hover/"acceptance.json").read_text()),
          "dimension_anchor_admission":json.loads((dimension_anchors/"acceptance.json").read_text()),
          "dimension_anchor_reopens":json.loads((dimension_anchors/"reopen-acceptance.json").read_text()),
          "fresh_source_acquisitions":0,"result":"pass"}
(out/"result.json").write_text(json.dumps(result,indent=2)+"\n")
print(json.dumps(result))
