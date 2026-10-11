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
from consumer_stages import Stages

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("binaries", type=Path, help="prebuilt example directory")
parser.add_argument("output", type=Path)
args = parser.parse_args()
repo = Path(__file__).resolve().parents[1]
binaries = args.binaries.resolve()
out = args.output.resolve()
out.mkdir(parents=True, exist_ok=True)
stages = Stages(out)
archive_dependencies = {}
pin = "6be29fe437a7cd992d4e079f71720821b25216b9"
assert subprocess.check_output(["git", "-C", str(repo/"ingetrazo"), "rev-parse", "HEAD"], text=True).strip() == pin
suffix = ".exe" if os.name == "nt" else ""
consumers = ["change_axes_fixture", "document_preservation_workflow", "pushpull_solid_workflows", "fillet_scoped", "pushpull_finite_workflows", "paste_save_workflow", "box_selection_fixture", "material_restamp_workflow", "material_tray_egui", "cursor_navigation_workflow", "annotation_delete_fixture", "zoom_extents_workflow", "annotation_move_fixture", "annotation_acquisition_fixture", "mesh_kernel_acceptance", "annotation_edit_style_fixture", "annotation_text_create_fixture", "mesh_finite_kernel_fixture", "annotation_hover_fixture", "dimension_anchor_acceptance_fixture", "context_document_lifecycle", "line_command_history_fixture", "shape_drafting_workflows", "freehand_workflows", "p07_inference_workflows", "perspective_guide_workflows", "p07_keyboard_workflows"]
identities = {name: stages.stage("binary:"+name, [], lambda name=name: hashlib.sha256((binaries/(name+suffix)).read_bytes()).hexdigest()) for name in consumers}
env = dict(os.environ, QT_QPA_PLATFORM="offscreen", INGETRAZO_STATE_DIR=str(out/"state"),
           XDG_CONFIG_HOME=str(out/"config"), XDG_DATA_HOME=str(out/"data"), XDG_CACHE_HOME=str(out/"cache"))

def _archive(name, manifest_name, selected, destination, preserve_paths=False, archive_name="records.zip"):
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

def archive(name, manifest_name, selected, destination, preserve_paths=False, archive_name="records.zip", stage_name=None):
    key = "archive:"+(stage_name or name)
    archive_dependencies.setdefault(destination.name, []).append(key)
    return stages.stage(key, [], lambda: _archive(name, manifest_name, selected, destination, preserve_paths, archive_name))

def run(argv, output, requires=()):
    dependencies = list(archive_dependencies.get(Path(output).parts[0], [])) + list(requires)
    if Path(argv[0]).parent == binaries:
        dependencies.append("binary:"+Path(argv[0]).stem)
    def execute():
        (out/output).parent.mkdir(parents=True, exist_ok=True)
        with (out/output).open("w") as stdout, (out/(output+".stderr")).open("w") as stderr:
            subprocess.run([str(v) for v in argv], cwd=repo, env=env, stdout=stdout, stderr=stderr, check=True, timeout=120)
    return stages.stage(output, dependencies, execute)

axes = out/"axes"
axis_cases = ["classic", "shared", "nested", "oblique", "context"]
a = archive("change-axes", "provenance.json", [c+".input.igz" for c in axis_cases]+["original.json"], axes)
doc = out/"documents"
d = archive("document-preservation-workflow", "manifest.json", ["capture/retained.igz", "capture/nested-leaves.igz", "capture/expected.json"], doc)
run([binaries/(consumers[0]+suffix), axes], "axes/native.json")
run([sys.executable, repo/"scripts/reference-change-axes.py", axes, "inspect"], "axes/reopened.json", requires=['axes/native.json'])
run([sys.executable, repo/"scripts/check-change-axes.py", axes], "axes/check.json", requires=['axes/native.json', 'axes/reopened.json'])
run([binaries/(consumers[1]+suffix), doc], "documents/native.json")
run([sys.executable, repo/"scripts/reference-document-preservation-workflow.py", "compare", doc], "documents/compare.json", requires=['documents/native.json'])
run([sys.executable, repo/"scripts/check-document-preservation-workflow.py", doc/"observed.json"], "documents/check.log", requires=['documents/compare.json'])
fillet = out/"fillet"
f = archive("fillet-nested-history", "provenance.json", ["source.json", "source-app.json", "old-native.json"], fillet)
run([binaries/(consumers[3]+suffix), fillet/"source.json", fillet/"native.json"], "fillet/runtime.log")
run([sys.executable, repo/"scripts/check-fillet-nested-history.py", fillet], "fillet/check.json", requires=['fillet/runtime.log'])
run([sys.executable, repo/"scripts/check-fillet-independent-volume.py", repo/"docs/audits", fillet/"volume.json", fillet/"native.json"], "fillet/volume.log", requires=['fillet/runtime.log'])
run([binaries/(consumers[2]+suffix), repo/"docs/audits/pushpull-solid-workflows/app-input.json", out/"pushpull-app.json"], "pushpull-runtime.log")
run([sys.executable, repo/"scripts/check-pushpull-solid-workflows.py", repo/"docs/audits/pushpull-solid-workflows", out/"pushpull-comparison.json", out/"pushpull-app.json"], "pushpull-check.log", requires=['pushpull-runtime.log'])
finite = out/"finite"
p = archive("pushpull-finite-corpus", "provenance.json", ["source.json"], finite)
run([binaries/(consumers[4]+suffix), finite/"source.json", finite/"native.json"], "finite/runtime.log")
run([sys.executable, repo/"scripts/check-pushpull-finite-corpus.py", repo/"docs/audits/pushpull-finite-corpus", finite/"comparison.json", finite/"native.json"], "finite/check.log", requires=['finite/runtime.log'])
paste = out/"paste-save"
ps = archive("paste-save", "provenance.json", ["input.igz", "original.json"], paste)
run([binaries/(consumers[5]+suffix), paste], "paste-save/runtime.log")
run([sys.executable, repo/"scripts/reference-paste-save.py", paste, "inspect"], "paste-save/reopen.log", requires=['paste-save/runtime.log'])
run([sys.executable, repo/"scripts/check-paste-save.py", paste], "paste-save/acceptance.json", requires=['paste-save/runtime.log', 'paste-save/reopen.log'])
box = out/"box-selection"
bs = archive("box-selection", "provenance.json", ["input.igz", "original.json", "orphan-input.igz", "guide-input.igz", "failures/missing-guides-native.json", "failures/face-layer-gate-native.json"], box, preserve_paths=True)
run([binaries/(consumers[6]+suffix), box], "box-selection/runtime.log")
run([sys.executable, repo/"scripts/reference-box-guides.py", "reopen", box], "box-selection/reopen.log", requires=['box-selection/runtime.log'])
run([sys.executable, repo/"scripts/check-box-selection.py", box], "box-selection/acceptance.json", requires=['box-selection/runtime.log', 'box-selection/reopen.log'])
material = out/"material-edit-restamp"
mr = archive("material-edit-restamp", "provenance.json", [], material)
run([binaries/(consumers[7]+suffix), material], "material-edit-restamp/runtime.log")
run([binaries/(consumers[8]+suffix), material], "material-edit-restamp/tray-runtime.log")
run([sys.executable, repo/"scripts/reopen-material-restamp.py", material, material/"reopened.json"], "material-edit-restamp/reopen.log", requires=['material-edit-restamp/runtime.log', 'material-edit-restamp/tray-runtime.log'])
run([sys.executable, repo/"scripts/check-material-restamp.py", "--native", material/"native.json", "--tray", material/"tray.json", "--reopened", material/"reopened.json"], "material-edit-restamp/acceptance.json", requires=['material-edit-restamp/runtime.log', 'material-edit-restamp/tray-runtime.log', 'material-edit-restamp/reopen.log'])
cursor = out/"cursor-pivot-navigation"
cn = archive("cursor-pivot-navigation", "manifest.json", ["original.json", "old-dispatch-native.json"], cursor)
run([binaries/(consumers[9]+suffix), cursor/"original.json", cursor/"native.json"], "cursor-pivot-navigation/runtime.log")
run([sys.executable, repo/"scripts/check-cursor-navigation.py", cursor/"original.json", cursor/"native.json"], "cursor-pivot-navigation/acceptance.json", requires=['cursor-pivot-navigation/runtime.log'])
def reject_old_cursor():
    with (cursor/"old-dispatch-rejection.log").open("w") as failure:
        rejected = subprocess.run([sys.executable, str(repo/"scripts/check-cursor-navigation.py"), str(cursor/"original.json"), str(cursor/"old-dispatch-native.json")], cwd=repo, env=env, stdout=failure, stderr=failure, timeout=120)
    assert rejected.returncode != 0, "Compiled original cursor dispatcher unexpectedly passed"
stages.stage("cursor-pivot-navigation/old-dispatch-rejection.log", archive_dependencies[cursor.name], reject_old_cursor)
annotations = out/"annotation-delete"
ad = archive("annotation-delete", "manifest.json", ["delete-input.igz", "original.json"]+[name+"-original.igz" for name in ["root", "nested-annotations", "nested-mixed", "parked"]], annotations)
ahd = archive("annotation-hover-delete", "manifest.json", ["hover-input.igz", "hover-delete-original.json"], annotations)
run([binaries/(consumers[10]+suffix), annotations], "annotation-delete/runtime.log")
run([sys.executable, repo/"scripts/check-annotation-delete.py", annotations], "annotation-delete/acceptance.json", requires=['annotation-delete/runtime.log'])
run([sys.executable, repo/"scripts/check-annotation-delete-reopen.py", annotations], "annotation-delete/reopen-acceptance.json", requires=['annotation-delete/runtime.log'])
zoom = out/"zoom-extents"
zoom_cases = ["parallel", "perspective", "two-point", "portrait", "hidden-nested", "empty", "orphan", "figure-only", "survey-only", "survey-hidden", "hidden-wire", "hidden-face"]
ze = archive("zoom-extents", "provenance.json", ["capture/source.json"]+["capture/"+case+".igz" for case in zoom_cases], zoom)
run([binaries/(consumers[11]+suffix), zoom], "zoom-extents/runtime.log")
run([sys.executable, repo/"scripts/check-zoom-extents.py", "--native", zoom/"native.json"], "zoom-extents/acceptance.json", requires=['zoom-extents/runtime.log'])
move = out/"annotation-move"
am = archive("annotation-move", "manifest.json", ["input.igz", "move-input.igz", "one-riding-input.igz", "original.json", "annotation-move-original-first-failure.json", "annotation-move-native-first-failure.json"], move)
run([binaries/(consumers[12]+suffix), move], "annotation-move/runtime.log")
run([sys.executable, repo/"scripts/check-annotation-move.py", move], "annotation-move/acceptance.json", requires=['annotation-move/runtime.log'])
acquisition = out/"annotation-acquisition"
aa = archive("annotation-acquisition/riding-order", "manifest.json", ["input.igz", "riding-input.igz", "original.json", "threshold-first-failure.json", "source-first-failure.log"], acquisition, archive_name="source-records.zip")
run([binaries/(consumers[13]+suffix), acquisition], "annotation-acquisition/runtime.log")
run([sys.executable, repo/"scripts/check-annotation-acquisition.py", acquisition], "annotation-acquisition/acceptance.json", requires=['annotation-acquisition/runtime.log'])
mesh = out/"mesh-kernel"
mk = archive("p05-mesh-kernel", "manifest.json", ["input.json", "source.json"], mesh)
run([binaries/(consumers[14]+suffix), mesh/"input.json"], "mesh-kernel/native.json")
run([sys.executable, repo/"scripts/check-mesh-kernel.py", mesh/"source.json", mesh/"native.json"], "mesh-kernel/acceptance.json", requires=['mesh-kernel/native.json'])
annotation_edit = out/"annotation-edit-style"
ae = archive("annotation-edit-style", "manifest.json", ["input.igz", "original.json", "failed-nested-style.json", "source-label-priority.json"] + [kind+"-"+context+".igz" for kind in ["label", "dimension"] for context in ["root", "nested"]], annotation_edit)
run([binaries/(consumers[15]+suffix), annotation_edit], "annotation-edit-style/runtime.log")
run([sys.executable, repo/"scripts/reopen-annotation-edit-style.py", annotation_edit], "annotation-edit-style/reopen.log", requires=['annotation-edit-style/runtime.log'])
run([sys.executable, repo/"scripts/check-annotation-edit-style.py", annotation_edit], "annotation-edit-style/acceptance.json", requires=['annotation-edit-style/runtime.log', 'annotation-edit-style/reopen.log'])
annotation_text = out/"annotation-text-create"
text_cases = [context+"-"+kind for context in ["root", "nested"] for kind in ["edge", "face", "coordinates"]]+["closed-face"]
at = archive("annotation-text-create", "manifest.json", ["input.igz", "original.json", "original-history.json", "failed-unknown-label-root-edge.igz", "failed-group-edge-suggestion.json"]+[name+ending for name in text_cases for ending in ["-input.igz", ".igz"]], annotation_text)
run([binaries/(consumers[16]+suffix), annotation_text], "annotation-text-create/runtime.log")
run([sys.executable, repo/"scripts/reopen-annotation-text-create.py", annotation_text], "annotation-text-create/reopen.log", requires=['annotation-text-create/runtime.log'])
run([sys.executable, repo/"scripts/check-annotation-text-create.py", annotation_text], "annotation-text-create/acceptance.jsonl", requires=['annotation-text-create/runtime.log', 'annotation-text-create/reopen.log'])
finite_mesh = out/"finite-kernel"
finite_names = ["empty", "one", "two", "repeated", "welded-close", "empty-hole", "one-hole", "two-hole", "empty-outer-proper-hole"]
mf = archive("p05-finite-kernel", "manifest.json", ["original.json"]+[name+".original.igz" for name in finite_names], finite_mesh)
mfa = archive("p05-finite-acceptance", "manifest.json", ["initial-native.json"], finite_mesh)
run([binaries/(consumers[17]+suffix), finite_mesh], "finite-kernel/runtime.log")
run([sys.executable, repo/"scripts/reopen-p05-finite-kernel.py", finite_mesh], "finite-kernel/reopen.log", requires=['finite-kernel/runtime.log'])
run([sys.executable, repo/"scripts/check-p05-finite-invariants.py", finite_mesh, "--initial", finite_mesh/"initial-native.json", "--initial-sha256", "519bc52b68587c56ed3c4f5683d7945ad9de24bd0e862c808b716b2eb9237aa9", "--report", finite_mesh/"acceptance.json"], "finite-kernel/checker.log", requires=['finite-kernel/runtime.log', 'finite-kernel/reopen.log'])
annotation_hover = out/"annotation-hover"
ah = archive("annotation-hover", "manifest.json", ["input.igz", "original-hover.json", "first40-native-weak.json"], annotation_hover)
run([binaries/(consumers[18]+suffix), annotation_hover], "annotation-hover/runtime.log")
run([sys.executable, repo/"scripts/check-annotation-hover.py", annotation_hover], "annotation-hover/acceptance.json", requires=['annotation-hover/runtime.log'])
dimension_anchors = out/"dimension-anchor-admission"
anchor_cases = ["near", "outside", "nested-shared", "billboard", "orphan"]
da = archive("dimension-anchor-admission", "manifest.json", [name+ending for name in anchor_cases for ending in ["-input.igz", "-original.igz"]]+["admission-original.json", "first-native-inverse-mismatch.json"], dimension_anchors)
run([binaries/(consumers[19]+suffix), dimension_anchors, "--admission"], "dimension-anchor-admission/runtime.log")
run([sys.executable, repo/"scripts/check-dimension-anchor-admission.py", dimension_anchors], "dimension-anchor-admission/acceptance.json", requires=['dimension-anchor-admission/runtime.log'])
run([sys.executable, repo/"scripts/reopen-dimension-anchor-admission.py", dimension_anchors], "dimension-anchor-admission/reopen-acceptance.json", requires=['dimension-anchor-admission/runtime.log'])
dimension_preview = out/"dimension-preview"
preview_cases = ["front-z", "side-x", "aligned", "axial-x", "axial-y", "nested"]
dp = archive("dimension-preview", "manifest.json", [name+ending for name in preview_cases for ending in ["-input.igz", "-original.igz"]]+["preview-original.json", "initial-native.json"], dimension_preview)
run([binaries/(consumers[19]+suffix), dimension_preview, "--preview"], "dimension-preview/runtime.log")
run([sys.executable, repo/"scripts/check-dimension-preview.py", dimension_preview], "dimension-preview/acceptance.json", requires=['dimension-preview/runtime.log'])
run([sys.executable, repo/"scripts/reopen-dimension-preview.py", dimension_preview], "dimension-preview/reopen-acceptance.json", requires=['dimension-preview/runtime.log'])
recent = out/"open-recent"
rr = archive("open-recent", "manifest.json", ["original-recent.json"], recent)
run([binaries/(consumers[20]+suffix), recent, "--recent"], "open-recent/runtime.log")
run([sys.executable, repo/"scripts/check-open-recent.py", recent], "open-recent/acceptance.json", requires=['open-recent/runtime.log'])
general_preferences = out/"general-preferences"
gp = archive("general-preferences", "manifest.json", ["original-preferences.json"], general_preferences)
run([binaries/(consumers[20]+suffix), general_preferences, "--preferences"], "general-preferences/runtime.log")
run([sys.executable, repo/"scripts/check-general-preferences.py", general_preferences], "general-preferences/acceptance.json", requires=['general-preferences/runtime.log'])
unit_preferences = out/"units-preferences"
unit_codes = ["m", "cm", "mm", "in", "ft", "ft-in", "in-frac", "ft-in-frac"]
up = archive("units-preferences", "manifest.json", ["original-units.json", "input.igz"]+[code+".igz" for code in unit_codes], unit_preferences)
run([binaries/(consumers[20]+suffix), unit_preferences, "--units"], "units-preferences/runtime.log")
run([sys.executable, repo/"scripts/check-units-preferences.py", unit_preferences], "units-preferences/acceptance.json", requires=['units-preferences/runtime.log'])
run([sys.executable, repo/"scripts/reopen-units-preferences.py", unit_preferences], "units-preferences/reopen.log", requires=['units-preferences/runtime.log'])
measurements = out/"vcb-measurements"
vm = archive("vcb-measurements", "manifest.json", ["original-measurements.json"], measurements)
run([binaries/(consumers[21]+suffix), measurements, "--measurements"], "vcb-measurements/runtime.log")
run([sys.executable, repo/"scripts/check-vcb-measurements.py", measurements], "vcb-measurements/acceptance.json", requires=['vcb-measurements/runtime.log'])
import_preferences = out/"import-preferences"
ip = archive("import-preferences", "manifest.json", ["original-import-preferences.json", "ambiguous.obj", "large.obj", "huge.obj", "header.dxf", "lying.dxf", "tiny.dxf"], import_preferences)
run([binaries/(consumers[20]+suffix), import_preferences, "--import-preferences"], "import-preferences/runtime.log")
run([sys.executable, repo/"scripts/check-import-preferences.py", import_preferences], "import-preferences/acceptance.json", requires=["import-preferences/runtime.log"])

obj_mtl = out/"obj-mtl-import"
obj_members = [row["path"] for row in json.loads((repo/"docs/audits/obj-mtl-import/source-manifest.json").read_text())["entries"]]
om = archive("obj-mtl-import", "source-manifest.json", obj_members, obj_mtl, preserve_paths=True, archive_name="source-records.zip", stage_name="obj-mtl-source")
run([binaries/(consumers[20]+suffix), obj_mtl, "--obj-mtl"], "obj-mtl-import/runtime.log")
run([sys.executable, repo/"scripts/reference-obj-mtl-reopens.py", obj_mtl], "obj-mtl-import/reopen.log", requires=["obj-mtl-import/runtime.log"])
run([sys.executable, repo/"scripts/check-obj-mtl-import.py", obj_mtl], "obj-mtl-import/acceptance.json", requires=["obj-mtl-import/runtime.log", "obj-mtl-import/reopen.log"])

obj_fusion = out/"obj-fusion"
fusion_members = [row["path"] for row in json.loads((repo/"docs/audits/obj-fusion/source-manifest.json").read_text())["entries"]]
of = archive("obj-fusion", "source-manifest.json", fusion_members, obj_fusion, preserve_paths=True, archive_name="source-records.zip", stage_name="obj-fusion-source")
run([binaries/(consumers[20]+suffix), obj_fusion, "--obj-fusion"], "obj-fusion/runtime.log")
run([sys.executable, repo/"scripts/reference-obj-fusion-reopens.py", obj_fusion], "obj-fusion/reopen.log", requires=["obj-fusion/runtime.log"])
# This finite gate reports its strict unresolved winding boundary explicitly;
# it does not promote P22 or silently accept the complete twelve-case corpus.
run([sys.executable, repo/"scripts/check-obj-fusion.py", obj_fusion, "--deterministic"], "obj-fusion/acceptance.json", requires=["obj-fusion/runtime.log", "obj-fusion/reopen.log"])

obj_raw = out/"obj-raw-fusion"
raw_members = [row["path"] for row in json.loads((repo/"docs/audits/obj-raw-fusion/source-manifest.json").read_text())["entries"]]
oraw = archive("obj-raw-fusion", "source-manifest.json", raw_members, obj_raw, preserve_paths=True, archive_name="source-records.zip", stage_name="obj-raw-source")
oraw_failure = archive("obj-raw-fusion", "manifest.json", ["attempts/preliminary-mesh/native-obj-fusion.json"], obj_raw, preserve_paths=True, stage_name="obj-raw-retained-failure")
run([binaries/(consumers[20]+suffix), obj_raw, "--obj-raw-fusion"], "obj-raw-fusion/runtime.log")
run([sys.executable, repo/"scripts/reference-obj-raw-fusion-reopens.py", obj_raw], "obj-raw-fusion/reopen.log", requires=["obj-raw-fusion/runtime.log"])
run([sys.executable, repo/"scripts/check-obj-raw-fusion.py", obj_raw], "obj-raw-fusion/acceptance.json", requires=["obj-raw-fusion/runtime.log", "obj-raw-fusion/reopen.log"])

# Reuse the same context consumer and authenticated source packages for both DXF subsets.
dxf_archives = {}
dxf_results = {}
for lane, package, mode, reader, checker in [
    ("dxf-entities", "dxf-legacy-polylines", "--dxf-entities", "reference-dxf-entities-reopens.py", "check-dxf-entities.py"),
    ("dxf-curves", "dxf-curves", "--dxf-curves", "reference-dxf-curves-reopens.py", "check-dxf-source-curves.py"),
]:
    folder = out/lane
    members = [row["path"] for row in json.loads((repo/"docs/audits"/package/"source-manifest.json").read_text())["entries"]]
    dxf_archives[lane] = archive(package, "source-manifest.json", members, folder, preserve_paths=True, archive_name="source-records.zip", stage_name=lane+"-source")
    run([binaries/(consumers[20]+suffix), folder, mode], lane+"/runtime.log")
    run([sys.executable, repo/"scripts"/reader, folder], lane+"/reopen.log", requires=[lane+"/runtime.log"])
    run([sys.executable, repo/"scripts"/checker, folder], lane+"/acceptance.json", requires=[lane+"/runtime.log", lane+"/reopen.log"])
    dxf_results[lane] = folder/"acceptance.json"

nested_import = out/"nested-import"
nested_cases = [context+"-"+asset for context in ["root", "container", "leaf"] for asset in ["ambiguous", "named", "parts", "header", "zero"]]
ni = archive("nested-import", "source-manifest.json", ["original-nested-import.json", "ambiguous.obj", "named.obj", "parts.obj", "header.dxf", "zero.dxf"]+[case+ending for case in nested_cases for ending in ["-input.igz", "-original.igz"]], nested_import, archive_name="source-records.zip", stage_name="nested-import-source")
nt = archive("nested-import", "threshold-source-manifest.json", ["original-threshold.json"]+["threshold-"+count+ending for count in ["400", "401"] for ending in ["-input.igz", ".obj"]], nested_import, archive_name="threshold-source-records.zip", stage_name="nested-import-threshold")
nn = archive("nested-import", "naming-source-manifest.json", ["original-naming.json", "naming-input.igz", "naming-large.obj"], nested_import, archive_name="naming-source-records.zip", stage_name="nested-import-naming")
nf = archive("nested-import", "manifest.json", ["attempts/initial-f64-shareback/native-nested-import.json"], nested_import, preserve_paths=True, stage_name="nested-import-retained-failure")
run([binaries/(consumers[20]+suffix), nested_import, "--nested-import"], "nested-import/runtime.log")
run([sys.executable, repo/"scripts/reference-nested-import-reopens.py", nested_import], "nested-import/reopen.log", requires=["nested-import/runtime.log"])
run([sys.executable, repo/"scripts/check-nested-import.py", nested_import], "nested-import/acceptance.json", requires=["nested-import/runtime.log", "nested-import/reopen.log"])

appearance = out/"appearance-preferences"
ap = archive("appearance-preferences", "manifest.json", ["original-appearance.json", "source-icons.json"], appearance)
run([binaries/(consumers[20]+suffix), appearance, "--appearance"], "appearance-preferences/runtime.log")
run([sys.executable, repo/"scripts/check-appearance-preferences.py", appearance], "appearance-preferences/acceptance.json", requires=["appearance-preferences/runtime.log"])

shape = out/"shape-drafting"
shape_sources = ["source.json", "source-addendum.json", "source-remaining.json"]
sd = archive("shape-drafting", "manifest.json", shape_sources, shape)
def combine_shape_sources():
    cohorts = [json.loads((shape/name).read_text())["cases"] for name in shape_sources]
    assert [len(cases) for cases in cohorts] == [53, 7, 6], "shape source cohort inventory"
    cases = [case for cohort in cohorts for case in cohort]
    assert len({case["recipe"]["name"] for case in cases}) == len(cases), "duplicate shape recipe"
    assert sum(len(case["states"]) for case in cases) == 488, "shape phase inventory"
    (shape/"combined-source.json").write_text(json.dumps({"cases":cases}, indent=2)+"\n")
    return {"cases":len(cases), "phases":488, "original_cohorts":shape_sources}
stages.stage("shape-drafting/combined-source.json", archive_dependencies[shape.name], combine_shape_sources)
run([binaries/(consumers[22]+suffix), shape/"combined-source.json", shape/"native.json"], "shape-drafting/runtime.log", requires=["shape-drafting/combined-source.json"])
run([sys.executable, repo/"scripts/check-shape-drafting.py", shape/"combined-source.json", shape/"native.json", shape/"acceptance.json"], "shape-drafting/checker.log", requires=["shape-drafting/runtime.log", "shape-drafting/combined-source.json"])

freehand = out/"freehand-workflows"
fh = archive("freehand-workflows", "manifest.json", ["source-combined.json"], freehand)
run([binaries/(consumers[23]+suffix), freehand/"source-combined.json", freehand/"native.json"], "freehand-workflows/runtime.log")
run([sys.executable, repo/"scripts/check-freehand-workflows.py", freehand/"source-combined.json", freehand/"native.json", freehand/"acceptance.json"], "freehand-workflows/checker.log", requires=["freehand-workflows/runtime.log"])

inference = out/"p07-inference"
p07 = archive("p07-inference", "ci-source-manifest.json", ["source.json", "source-app.json", "production-callback.json"], inference, archive_name="source-records.zip")
run([binaries/(consumers[24]+suffix), inference/"source.json", inference/"source-app.json", inference/"native.json"], "p07-inference/runtime.log")
run([sys.executable, repo/"scripts/check-p07-inference.py", inference/"source.json", inference/"source-app.json", inference/"native.json", inference/"acceptance.json"], "p07-inference/checker.log", requires=["p07-inference/runtime.log"])

perspective_guides = out/"perspective-guides"
pg = archive("perspective-guide-workflows", "source-manifest.json", ["source.json"], perspective_guides, archive_name="source-records.zip")
run([binaries/(consumers[25]+suffix), perspective_guides/"source.json", perspective_guides/"native.json"], "perspective-guides/runtime.log")
run([sys.executable, repo/"scripts/check-perspective-guides.py", perspective_guides/"source.json", perspective_guides/"native.json", perspective_guides/"acceptance.json"], "perspective-guides/checker.log", requires=["perspective-guides/runtime.log"])


keyboard = out/"p07-keyboard"
pk = archive("p07-keyboard", "source-manifest.json", ["source.json"], keyboard, archive_name="source-records.zip")
run([binaries/(consumers[26]+suffix), keyboard/"source.json", keyboard/"native.json"], "p07-keyboard/runtime.log")
run([sys.executable, repo/"scripts/check-p07-keyboard.py", keyboard/"source.json", keyboard/"native.json", keyboard/"acceptance.json"], "p07-keyboard/checker.log", requires=["p07-keyboard/runtime.log"])


def summarize():
    rows = json.loads((doc/"native.json").read_text())["rows"]
    assert [(x["fixture"],x["phase"]) for x in rows] == [(f,p) for f in ["retained","nested-leaves"] for p in ["edited","undo","redo"]]
    assert all(x["saved"] and x["clean"] for x in rows)
    result = {"revision":subprocess.check_output(["git","rev-parse","HEAD"],cwd=repo,text=True).strip(),
              "source_pin":pin,"binary_sha256":identities,"source_archives":{"axes":a,"documents":d,"fillet":f,"pushpull_finite":p,"paste_save":ps,"box_selection":bs,"materials":mr,"cursor":cn,"annotations":ad,"annotation_hover_delete":ahd,"zoom_extents":ze,"annotation_move":am,"annotation_acquisition":aa,"mesh_kernel":mk,"annotation_edit_style":ae,"annotation_text_create":at,"finite_kernel":mf,"finite_kernel_initial":mfa,"annotation_hover":ah,"dimension_anchor_admission":da,"dimension_preview":dp,"open_recent":rr,"general_preferences":gp,"units_preferences":up,"vcb_measurements":vm,"import_preferences":ip,"appearance_preferences":ap,"nested_import":ni,"nested_import_threshold":nt,"nested_import_naming":nn,"nested_import_retained_failure":nf,"shape_drafting":sd,"freehand_workflows":fh,"obj_mtl_import":om,"obj_fusion":of,"p07_inference":p07,"perspective_guides":pg,"p07_keyboard":pk,"obj_raw_fusion":oraw,"obj_raw_retained_failure":oraw_failure},
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
              "dimension_preview":json.loads((dimension_preview/"acceptance.json").read_text()),
              "dimension_preview_reopens":json.loads((dimension_preview/"reopen-acceptance.json").read_text()),
              "open_recent":json.loads((recent/"acceptance.json").read_text()),
              "general_preferences":json.loads((general_preferences/"acceptance.json").read_text()),
              "units_preferences":json.loads((unit_preferences/"acceptance.json").read_text()),
              "unit_reopens":json.loads((unit_preferences/"reopen-units.json").read_text()),
              "vcb_measurements":json.loads((measurements/"acceptance.json").read_text()),
              "import_preferences":json.loads((import_preferences/"acceptance.json").read_text()),
              "appearance_preferences":json.loads((appearance/"acceptance.json").read_text()),
              "nested_import":json.loads((nested_import/"acceptance.json").read_text()),
              "obj_mtl_import":json.loads((obj_mtl/"acceptance.json").read_text()),
              "obj_fusion":json.loads((obj_fusion/"acceptance.json").read_text()),
              "obj_raw_fusion":json.loads((obj_raw/"acceptance.json").read_text()),
              "p07_keyboard":json.loads((keyboard/"acceptance.json").read_text()),
              "p07_inference":json.loads((inference/"acceptance.json").read_text()),
              "perspective_guides":json.loads((perspective_guides/"acceptance.json").read_text()),
              "shape_drafting":json.loads((shape/"acceptance.json").read_text()),
              "freehand_workflows":json.loads((freehand/"acceptance.json").read_text()),
              "dxf_source_archives":dxf_archives,"dxf":{lane:json.loads(path.read_text()) for lane,path in dxf_results.items()},
              "fresh_source_acquisitions":0,"result":"pass"}
    return result

sys.exit(stages.finish(summarize))
