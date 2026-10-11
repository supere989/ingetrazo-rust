"""Exact saved-light workflow/camera comparison; no pixel or numeric tolerance."""
import copy,json,sys
from pathlib import Path

def check(a,b):
    assert a==b, "source/native document, camera or follow key differs"

def main():
    source=json.loads(Path(sys.argv[1]).read_text());native=json.loads(Path(sys.argv[2]).read_text())
    check(source,native)
    rejected=[]
    changes=[("light-position",lambda b:b["states"][1]["document"]["lights"][0]["pos"].__setitem__(2,3.)),
      ("undo",lambda b:b["states"][4]["document"].__setitem__("ambience","night")),
      ("custom-rgb",lambda b:b["clean"][2]["lights"][0]["color"].__setitem__(0,0.3)),
      ("saved-power",lambda b:b["reopen"]["lights"][1].__setitem__("power",5)),
      ("camera",lambda b:b["cameras"][0]["camera"]["target"].__setitem__(0,0.1)),
      ("camera-key",lambda b:b["cameras"][0]["key"][0].__setitem__(0,77))]
    for name,change in changes:
        altered=copy.deepcopy(native);change(altered)
        try:check(source,altered)
        except AssertionError:rejected.append(name)
        else:raise AssertionError("undetected mutation: "+name)
    print(json.dumps(dict(exact=True,clean_cases=len(source["clean"]),document_states=len(source["states"]),camera_cases=len(source["cameras"]),native_reopened_by_source=True,negative_controls=rejected),indent=2))
if __name__=="__main__":main()
