#!/usr/bin/env python3
"""Strict ordered clipboard parity; only fresh identity spelling is relational."""
import argparse, copy, json, math, re
from pathlib import Path
PIN = "6be29fe437a7cd992d4e079f71720821b25216b9"
CASES = ["text-copy-cut-history", "bound-cohort", "cross-sheet-bindings", "title-replace-window", "all-fifteen", "locked-hidden-copy"]
IDENTITIES = {"uid", "frame_uid", "anchor_uid", "group_id"}
UUID = re.compile(r"[0-9a-f]{32}\Z")
CONTROLS = ["actual_clipboard_ingress", "clipboard_persistence", "atomic_refusal"]

def load(path):
    return json.loads(Path(path).read_text(), parse_constant=lambda x: (_ for _ in ()).throw(ValueError(x)))

def old_ids(value, found=None):
    found = set() if found is None else found
    if isinstance(value, dict):
        for k, v in value.items():
            if k in IDENTITIES and isinstance(v, str) and v: found.add(v)
            old_ids(v, found)
    elif isinstance(value, list):
        for v in value: old_ids(v, found)
    return found

class Comparator:
    def __init__(self, old):
        self.old = old
        self.forward = {}
        self.reverse = {}
    def equal(self, expected, actual, path="root", key=None):
        if key in IDENTITIES and isinstance(expected, str) and UUID.fullmatch(expected) and expected not in self.old:
            assert isinstance(actual, str) and UUID.fullmatch(actual), (path, "fresh identity syntax", actual)
            assert actual not in self.old, (path, "fresh identity aliases original", actual)
            assert self.forward.get(expected, actual) == actual, (path, "identity reference changed", expected, actual)
            assert self.reverse.get(actual, expected) == expected, (path, "identity bijection collision", expected, actual)
            self.forward[expected] = actual; self.reverse[actual] = expected
            return
        if isinstance(expected, bool) or expected is None or isinstance(expected, str):
            assert type(expected) is type(actual) and expected == actual, (path, expected, actual)
        elif isinstance(expected, (int, float)):
            assert type(actual) in (int, float) and math.isfinite(actual) and actual == expected, (path, expected, actual)
            if expected == 0: assert math.copysign(1., expected) == math.copysign(1., actual), (path, "signed zero", expected, actual)
        elif isinstance(expected, list):
            assert isinstance(actual, list) and len(expected) == len(actual), (path, "ordered length", len(expected), len(actual) if isinstance(actual,list) else type(actual).__name__)
            for i, (a,b) in enumerate(zip(expected,actual)): self.equal(a,b,f"{path}[{i}]")
        elif isinstance(expected, dict):
            assert isinstance(actual, dict) and expected.keys() == actual.keys(), (path, "keys", sorted(expected), sorted(actual) if isinstance(actual,dict) else type(actual).__name__)
            for k,v in expected.items(): self.equal(v,actual[k],f"{path}.{k}",k)
        else: raise AssertionError((path,"unsupported type"))

def expected_controls(source):
    rows=[]
    for case in source["cases"]:
        states=[s for s in source["states"] if s["case"]==case["name"]]
        before=states[0];counts={"clipboard_events":0,"operative_copies":0,"operative_cuts":0,"operative_pastes":0}
        for action,after in zip(case["actions"],states):
            if action["op"]=="clipboard":
                counts["clipboard_events"]+=1;k=action["key"]
                if k=="C" and before["selection"]: counts["operative_copies"]+=1
                if k=="X" and before["selection"]: counts["operative_cuts"]+=1
                if k=="V" and before["clipboard"]: counts["operative_pastes"]+=1
            before=after
        rows.append({"case":case["name"],"actual_clipboard_ingress":True,**counts,"atomic_refusal":bool(states[-1]["clipboard"]),"clipboard_persistence":any(a["op"]=="reopen_window" for a in case["actions"])})
    return rows

def inventory(source, native, readers):
    assert source["source_pin"] == PIN and native["source_pin"] == PIN
    assert [c["name"] for c in source["cases"]] == CASES
    assert [c["name"] for c in native["cases"]] == CASES
    expected = []
    for c in source["cases"]: expected.extend([(c["name"],a["phase"]) for a in c["actions"]]+[(c["name"],"save")])
    assert len(expected) == 68 and [(s["case"],s["phase"]) for s in source["states"]] == expected
    assert [(s["case"],s["phase"]) for s in native["states"]] == expected
    assert [r["case"] for r in readers] == CASES
    assert native.get("failures",[]) == []
    for flag in CONTROLS: assert native["controls"][flag] is True, flag
    expected=expected_controls(source)
    assert [c["case"] for c in native["control_cases"]]==CASES
    for want,got in zip(expected,native["control_cases"]):
        for key,v in want.items(): assert type(got[key]) is type(v) and got[key]==v,("control_cases",want["case"],key,v,got[key])
    for state in native["states"]:
        for field in ("sheet_index","undo","redo"):
            assert type(state[field]) is int and state[field] >= 0, field
        assert state["clipboard_from"] is None or (type(state["clipboard_from"]) is int and state["clipboard_from"] >= 0)
        for field in ("selection", "selection_order"):
            for owner in state[field]: assert type(owner["index"]) is int and owner["index"] >= 0
        if state["panel_owner"] is not None: assert type(state["panel_owner"]["index"]) is int and state["panel_owner"]["index"] >= 0
        for row in state["clipboard"]: assert type(row["category"]) is int and 0 <= row["category"] < 15

def validate(source, native, original_readers, native_readers):
    inventory(source,native,native_readers)
    assert [r["case"] for r in original_readers] == CASES
    comparators = {c:Comparator(old_ids(next(s["all_compositions"] for s in source["states"] if s["case"] == c and s["phase"] == "initial"))) for c in CASES}
    diffs=[]; order=[]
    for i,(expected,actual) in enumerate(zip(source["states"],native["states"])):
        if expected["selection_order"] != actual["selection_order"]: order.append({"case":expected["case"],"phase":expected["phase"],"source":expected["selection_order"],"native":actual["selection_order"]})
        comparator=copy.deepcopy(comparators[expected["case"]])
        try: comparator.equal(expected,actual,f"states[{i}]")
        except AssertionError as e: diffs.append({"case":expected["case"],"phase":expected["phase"],"difference":repr(e.args)})
        else: comparators[expected["case"]]=comparator
    reader_diffs=[]
    for expected,actual in zip(original_readers,native_readers):
        try: comparators[expected["case"]].equal(expected,actual,"reader")
        except AssertionError as e: reader_diffs.append({"case":expected["case"],"difference":repr(e.args)})
    return {"ok":not diffs and not reader_diffs,"state_mismatches":diffs,"reader_mismatches":reader_diffs,"selection_order_differences":order,"identity_pairs":{c:dict(v.forward) for c,v in comparators.items()}}

def rejects(source,native,original,readers):
    controls=[]
    def attempt(name,mutate,reader=False):
        n=copy.deepcopy(native);r=copy.deepcopy(readers);mutate(r if reader else n)
        try: result=validate(source,n,original,r); rejected=not result["ok"]
        except (AssertionError,KeyError,TypeError,ValueError,IndexError): rejected=True
        assert rejected,("nonrejecting control",name)
        controls.append(name)
    attempt("missing-state",lambda n:n["states"].pop())
    attempt("reordered-states",lambda n:n["states"].reverse())
    attempt("history-bool",lambda n:n["states"][0].update(undo=True))
    attempt("history",lambda n:n["states"][0].update(undo=99))
    attempt("system-text",lambda n:n["states"][0].update(system_text="invented"))
    attempt("operative-cut-count",lambda n:n["control_cases"][2].update(operative_cuts=0))
    attempt("empty-hidden-refusal-claim",lambda n:n["control_cases"][-1].update(atomic_refusal=True))
    for flag in CONTROLS: attempt(flag,lambda n,f=flag:n["controls"].update({f:False}))
    nonempty=next(i for i,s in enumerate(native["states"]) if s["clipboard"])
    attempt("clipboard-model",lambda n:n["states"][nonempty]["clipboard"][0]["record"].update(x_mm=999.))
    multiple=next(i for i,s in enumerate(native["states"]) if len(s["clipboard"])>1)
    attempt("clipboard-order",lambda n:n["states"][multiple]["clipboard"].reverse())
    selected=next(i for i,s in enumerate(native["states"]) if len(s["selection_order"])>1)
    attempt("selection-order",lambda n:n["states"][selected]["selection_order"].reverse())
    pasted=next(i for i,s in enumerate(native["states"]) if s["phase"]=="paste-one" and s["case"]=="text-copy-cut-history")
    attempt("pasted-z",lambda n:n["states"][pasted]["composition"]["texts"][-1].update(z=999.))
    attempt("pasted-locked",lambda n:n["states"][pasted]["composition"]["texts"][-1].update(locked=True))
    generated=[]
    def walk(v,path,old):
        if isinstance(v,dict):
            for k,x in v.items():
                if k in IDENTITIES and isinstance(x,str) and UUID.fullmatch(x) and x not in old: generated.append((path+[k],x))
                walk(x,path+[k],old)
        elif isinstance(v,list):
            for i,x in enumerate(v): walk(x,path+[i],old)
    for i,state in enumerate(native["states"]):
        initial=next(v["all_compositions"] for v in source["states"] if v["case"]==state["case"] and v["phase"]=="initial")
        walk(state,["states",i],old_ids(initial))
    assert generated,"No operative fresh identity evidence"
    identity_path=generated[0][0]
    def set_path(n,path,value):
        for k in path[:-1]: n=n[k]
        n[path[-1]]=value
    attempt("fresh-identity-syntax",lambda n:set_path(n,identity_path,"not-a-fresh-uuid"))
    attempt("fresh-identity-aliases-original",lambda n:set_path(n,identity_path,"copy-frame"))
    attempt("missing-reader",lambda r:r.pop(),True)
    attempt("reader-model",lambda r:r[0]["compositions"][0]["texts"][0].update(text="wrong"),True)
    return controls

def main():
    p=argparse.ArgumentParser();p.add_argument("source",type=Path);p.add_argument("native",type=Path);p.add_argument("comparison",type=Path);a=p.parse_args()
    result={"ok":False}
    try:
        source=load(a.source/"original-clipboard.json");native=load(a.native/"native-clipboard.json")
        original=load(a.source/"original-reopens.json");readers=load(a.native/"native-reopens.json")
        result=validate(source,native,original,readers)
        result.update(cases=6,states=68,original_reader_pairs=6,scope="Strict ordered source states and models; fresh UID/group spelling only is bijective. z/clipboard/selection order remain strict. Native session-reset persistence is not native window-close proof.",source_noops=["locked-hidden-copy: force_select acquires none; Copy/Paste are no-ops", "text-copy-cut-history: Cut after history-cleared selection is a no-op"],operative_cut="cross-sheet-bindings Text+Cota",source_callback_errors=len(source["callback_errors"]))
        if result["ok"]: result["rejected_controls"]=rejects(source,native,original,readers)
    except Exception as e: result["error"]=repr(e)
    a.comparison.parent.mkdir(parents=True,exist_ok=True);a.comparison.write_text(json.dumps(result,indent=2)+"\n")
    assert result.get("ok") and "error" not in result,result
if __name__=="__main__": main()
