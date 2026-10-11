#!/usr/bin/env python3
"""Measurement-only source bundle receipt. This does not admit cached replay."""
import hashlib, importlib, importlib.metadata, json, os, platform, re, subprocess, sys
from pathlib import Path

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def environment(repo):
    import PySide6, numpy
    from PySide6.QtCore import qVersion
    source=(repo/"ingetrazo").resolve()
    revision=subprocess.check_output(["git","-C",str(source),"rev-parse","HEAD"],text=True).strip()
    subprocess.run(["git","-C",str(source),"diff","--quiet","HEAD"],check=True)
    # Hash actual Python inputs, including an untracked importable file if present.
    original={str(p.relative_to(source)):digest(p) for p in sorted(source.rglob("*.py")) if ".git" not in p.parts}
    helpers=["probe-python-sheet-view-pdf.py","reference-composer-hlr-fixture.py","probe-python-paper-boundary.py","probe-qt-paper-numerals.py","paper_source_receipt.py"]
    roots=set()
    for name in ["PySide6.QtCore","PySide6.QtGui","PySide6.QtWidgets","PySide6.QtPdf","numpy._core._multiarray_umath"]:
        roots.add(Path(importlib.import_module(name).__file__).resolve())
    qt=Path(PySide6.__file__).resolve().parent/"Qt"
    plugin=qt/"plugins/platforms/libqoffscreen.so"
    if sys.platform!="linux" or not plugin.is_file():
        raise RuntimeError("Source environment receipt currently requires the native Linux offscreen provider")
    roots.add(plugin.resolve())
    libraries=set(roots);pending=list(roots)
    while pending:
        path=pending.pop()
        output=subprocess.check_output(["ldd",str(path)],text=True,stderr=subprocess.STDOUT)
        if "not found" in output: raise RuntimeError("Missing source provider dependency: "+str(path))
        for line in output.splitlines():
            found=re.search(r"(?:=>\s*)?(/[^\s]+)",line)
            if found:
                dependency=Path(found[1]).resolve()
                if dependency not in libraries: libraries.add(dependency);pending.append(dependency)
    freeze=repo/"docs/audits/python-runtime-freeze.txt"
    packages={line.split("==")[0]:importlib.metadata.version(line.split("==")[0]) for line in freeze.read_text().splitlines() if "==" in line}
    numerical_files={}
    for name in ["numpy","mapbox_earcut","shapely"]:
        distribution=importlib.metadata.distribution(name)
        for file in distribution.files or []:
            if str(file).endswith(".so"):
                path=Path(distribution.locate_file(file)).resolve();numerical_files[str(path)]=digest(path)
    effective={"saved":os.environ.get("INGETRAZO_VIEW_PDF_SAVED")=="1","section_tilt":os.environ.get("INGETRAZO_VIEW_PDF_SECTION_TILT")=="1","fill":os.environ.get("INGETRAZO_VIEW_PDF_FILL","none"),"scale":float(os.environ.get("INGETRAZO_VIEW_PDF_SCALE","100")),"hatch":float(os.environ.get("INGETRAZO_VIEW_PDF_HATCH","1.5")),"fill_color":os.environ.get("INGETRAZO_VIEW_PDF_FILL_COLOR","#595e69"),"grid":float(os.environ.get("INGETRAZO_VIEW_PDF_GRID","0"))}
    runtime_environment={name:os.environ.get(name) for name in ["QT_QPA_PLATFORM","QT_SCALE_FACTOR","QT_SCREEN_SCALE_FACTORS","QT_FONT_DPI","QT_USE_PHYSICAL_DPI","QT_PLUGIN_PATH","LD_LIBRARY_PATH","LANG","LC_ALL","TZ","OPENBLAS_NUM_THREADS","OMP_NUM_THREADS"]}
    cpu={}
    cpu_path=Path("/proc/cpuinfo")
    if cpu_path.exists():
        for line in cpu_path.read_text().splitlines():
            if ":" in line:
                name,value=line.split(":",1)
                if name.strip() in ["vendor_id","model name","flags"]:cpu.setdefault(name.strip(),value.strip())
    identity={"schema":1,"runtime_environment":runtime_environment,"cpu":cpu,"source_commit":revision,"source_python":original,"helpers":{name:digest(repo/"scripts"/name) for name in helpers},"recipe":effective,"python":sys.version,"machine":platform.machine(),"system":platform.platform(),"qt":qVersion(),"pyside":PySide6.__version__,"numpy":numpy.__version__,"platform_plugin":os.environ.get("QT_QPA_PLATFORM","offscreen"),"provider_discovery":"recursive ldd closure of Qt/PDFium, NumPy and offscreen plugin","provider_files":{str(p):digest(p) for p in sorted(libraries)},"runtime_freeze_sha256":digest(freeze),"installed_packages":packages,"numerical_extension_files":numerical_files,"font_scope":"No text ink: source vector frames disable annotations/title; numeral controls draw paths only"}
    encoded=json.dumps(identity,sort_keys=True,separators=(",",":"),allow_nan=False).encode()
    return {"identity":identity,"binding_sha256":hashlib.sha256(encoded).hexdigest()}

def outputs(root):
    paths=set((root/"corpus").glob("*.igz"))|set((root/"corpus").glob("*.reference.json"))
    for pattern in ["*.original.pdf","*.operative-paper.json","qt-number-*.pdf"]: paths.update(root.glob(pattern))
    for name in ["original-paper-boundary.json","qt-number-observations.json"]:
        path=root/name
        if not path.is_file(): raise RuntimeError("Missing generated source output: "+name)
        paths.add(path)
    if len(list(root.glob("qt-number-*.pdf")))!=8: raise RuntimeError("Expected eight Qt numeral controls")
    names=[p.name.removesuffix(".original.pdf") for p in root.glob("*.original.pdf")]
    if len(names)!=13 or len(set(names))!=13: raise RuntimeError("Expected thirteen source PDF fixtures")
    for name in names:
        for path in [root/(name+".operative-paper.json"),root/"corpus"/(name+".igz"),root/"corpus"/(name+".reference.json")]:
            if not path.is_file(): raise RuntimeError("Missing recipe source member: "+str(path))
    return [{"path":str(p.relative_to(root)),"bytes":p.stat().st_size,"sha256":digest(p)} for p in sorted(paths)]
