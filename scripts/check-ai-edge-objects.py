#!/usr/bin/env python3
"""Offline identical-recipe validation only, not a production Python adapter."""
import argparse,json,os,pathlib,subprocess,tempfile
p=argparse.ArgumentParser();p.add_argument("native");p.add_argument("--python",default="/tmp/ingetrazo-parity-python/bin/python");p.add_argument("--source",default="/work/ingetrazo");a=p.parse_args()
recipe=pathlib.Path(__file__).parent/"fixtures/ai-edge-objects.py"
text=recipe.read_text(); native=pathlib.Path(a.native)
if not native.is_file():raise SystemExit("Missing native fixture")
env=dict(os.environ,QT_QPA_PLATFORM="offscreen",PYTHONPATH=a.source)
loader="from core.mesh import Vertex,Edge\nfrom PySide6.QtGui import QVector3D\nimport sys\nexec(compile(open(sys.argv[1]).read(),sys.argv[1],'exec'))"
def validate(path):
    original=subprocess.run([a.python,"-c",loader,str(path)],env=env,capture_output=True,text=True,timeout=20)
    actual=subprocess.run([str(native),str(path)],capture_output=True,text=True,timeout=20)
    if actual.returncode==0:
        result=json.loads(actual.stdout)
        if result!={"assertions_passed":True,"production_enabled":False,"scope":"standalone-constructor-and-incidence-objects","final_owner_charge":0}:raise AssertionError("Unexpected native inventory")
    return original,actual
original,actual=validate(recipe)
assert original.returncode==actual.returncode==0,(original.stderr,actual.stderr)
changes={
"coordinate-equality-as-identity":("assert e.has(a) and e.has(b) and not e.has(c)","assert e.has(c)"),
"constructor-inserts-incidence":("assert not a.edges and not b.edges","assert e in a.edges and e in b.edges"),
"position-copy-instead-of-alias":("assert e.a is a.position and e.b is b.position","assert e.a is not a.position"),
"foreign-other-refused":("assert e.other(a) is b and e.other(b) is a and e.other(c) is a","assert e.other(c) is b"),
"radial-value-copy":("assert e.faces is replacement and held==[shared,shared,extra]","assert e.faces is not replacement"),
"slot-json-copy":("assert e.layer is payload and e.curve is payload and e.soft is payload and e.hidden is payload","assert e.layer is not payload"),
}
negatives={}
with tempfile.TemporaryDirectory(prefix="ai-edge-assertions-") as d:
    for name,(before,after) in changes.items():
        assert text.count(before)==1,name
        path=pathlib.Path(d)/(name+".py");path.write_text(text.replace(before,after))
        old,new=validate(path)
        assert old.returncode!=0 and new.returncode!=0,name
        assert "AssertionError" in old.stderr and "AssertionError" in new.stderr,name
        negatives[name]={"original_exit":old.returncode,"native_exit":new.returncode}
print(json.dumps({"positive":True,"negative_controls":negatives,"scope":"standalone-constructor-and-incidence-objects","production_enabled":False},indent=2))
