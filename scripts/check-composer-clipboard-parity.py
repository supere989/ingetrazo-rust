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

FAMILIES=["frames","texts","images","scalebars","nortes","leyendas","shapes","cotas","cotas_ang","cotas_rad","etiquetas","perfiles","niveles","llamadas","cajetin"]
def snapshot(comp,owner):
    family=owner["family"];assert family in FAMILIES
    row=comp[family] if family=="cajetin" else comp[family][owner["index"]]
    return {"category":FAMILIES.index(family),"record":copy.deepcopy(row)}
def ordered_items(comp):
    for family in FAMILIES:
        if family=="cajetin":
            if comp.get(family) is not None: yield comp[family]
        else: yield from comp.get(family,[])
def source_law(source,native,readers):
    # This validates the literal source transition conditional on the actual
    # native selection/clipboard sequence. It never converts strict FAIL to PASS.
    diffs=[];checked=0;serial=0;seen=set()
    def fresh():
        nonlocal serial
        serial+=1
        token=f"{serial:032x}"
        while token in seen: serial+=1;token=f"{serial:032x}"
        return token
    for case in source["cases"]:
        states=[v for v in native["states"] if v["case"]==case["name"]]
        before=states[0]
        for action,after in zip(case["actions"],states):
            seen.update(old_ids(before))
            if action["op"]!="clipboard": before=after;continue
            try:
                expected=copy.deepcopy(before);key=action["key"];comp=expected["composition"];clip=expected["clipboard"];index=before["sheet_index"]
                selected=before["selection_order"]
                # Actual acquisition order, not recorded Qt order or sorted z.
                if key in ("C","X") and selected:
                    clip=[snapshot(before["composition"],o) for o in selected]
                    expected["clipboard"]=clip;expected["clipboard_from"]=index
                    text=[r["record"]["text"] for r in clip if r["category"] in (1,10) and r["record"].get("text")]
                    if text: expected["system_text"]="\n".join(text)
                    if key=="X":
                        for family in FAMILIES:
                            indices={o["index"] for o in selected if o["family"]==family}
                            if not indices: continue
                            if family=="cajetin": comp.pop(family,None)
                            else:
                                comp[family]=[r for i,r in enumerate(comp[family]) if i not in indices]
                                if not comp[family] and family!="frames": comp.pop(family)
                        expected["selection"]=[];expected["selection_order"]=[];expected["panel_owner"]=None
                        expected["undo"]+=1;expected["redo"]=0
                elif key=="V" and clip:
                    step=5. if before["clipboard_from"]==index else 0.
                    z=max((r.get("z",0.) for r in ordered_items(comp)),default=-1.)+1.
                    here={r.get("uid","") for r in comp.get("frames",[]) if r.get("uid")};frames={};groups={};pasted=[];owners=[]
                    for entry in clip:
                        row=copy.deepcopy(entry["record"]);cat=entry["category"];family=FAMILIES[cat]
                        if cat!=14: row["x_mm"]+=step;row["y_mm"]+=step
                        row["locked"]=False
                        if row.get("group_id"): groups.setdefault(row["group_id"],fresh());row["group_id"]=groups[row["group_id"]]
                        if cat==0:
                            old=row.get("uid","");row["uid"]=fresh()
                            if old: frames[old]=row["uid"]
                        elif cat in (8,9,10,12,13): row["uid"]=""
                        row["z"]=z;z+=1.;pasted.append((family,row))
                    for family,row in pasted:
                        ref=row.get("frame_uid","")
                        if ref in frames: row["frame_uid"]=frames[ref]
                        elif ref and ref not in here: row["frame_uid"]=""
                        ref=row.get("anchor_uid","")
                        if ref:
                            row["anchor_uid"]=frames.get(ref,"")
                            if not row["anchor_uid"]:
                                row["a_world"]=None
                                if "b_world" in row: row["b_world"]=None
                        if family=="cajetin": comp[family]=row;i=0
                        else: i=len(comp.setdefault(family,[]));comp[family].append(row)
                        if not row.get("hidden",False): owners.append({"family":family,"index":i})
                    # Qt selectedItems has no promised iteration order. Membership
                    # stays exact; native order is separately retained by strict gate.
                    assert sorted(after["selection_order"],key=lambda o:(o["family"],o["index"]))==sorted(owners,key=lambda o:(o["family"],o["index"])),"pasted selection membership"
                    assert after["panel_owner"] is None if not owners else after["panel_owner"] in owners,"pasted shown owner"
                    expected["selection"]=sorted(owners,key=lambda o:(o["family"],o["index"]))
                    expected["selection_order"]=after["selection_order"];expected["panel_owner"]=after["panel_owner"]
                    expected["undo"]+=1;expected["redo"]=0
                    for entry,(_,row) in zip(clip,pasted):
                        if entry["category"]!=14:
                            entry["record"]["x_mm"]=row["x_mm"];entry["record"]["y_mm"]=row["y_mm"]
                    expected["clipboard_from"]=index
                expected["all_compositions"][index]=comp
                expected["phase"]=after["phase"]
                Comparator(seen).equal(expected,after,"source-law")
                checked+=1
            except (AssertionError,KeyError,TypeError,ValueError,IndexError) as e:
                diffs.append({"case":case["name"],"phase":after["phase"],"difference":repr(e.args)})
            before=after
        try:
            final=states[-1]["all_compositions"];reader=next(r for r in readers if r["case"]==case["name"])
            Comparator(old_ids(final)).equal(final,reader["compositions"],"native-original-reader")
        except (AssertionError,KeyError,TypeError,ValueError,IndexError) as e: diffs.append({"case":case["name"],"phase":"reader","difference":repr(e.args)})
    return {"ok":not diffs,"checked_clipboard_transitions":checked,"original_reader_pairs":len(readers),"mismatches":diffs,"scope":"Order-parametric literal source Copy/Cut/Paste laws driven by actual native selectedItems-equivalent order. Whole models, z, UID/group relations, offsets, clipboard and document preservation checked. Paste membership exact; unspecified selectedItems iteration/shown-member choice is reported by separate strict gate. Not exact captured output parity, Undo/Redo laws are outside this report."}

def traversal_residuals(source,native,original_readers,native_readers):
    """Finite unique-kind clipboard classifier; raw strict reports stay untouched."""
    assert source_law(source,native,native_readers)["ok"],"native transition law failed"
    assert source_law(source,source,original_readers)["ok"],"retained source transition law failed"
    mismatches=[]
    def strip_z(comp):
        comp=copy.deepcopy(comp)
        for family in FAMILIES:
            rows=[comp[family]] if family=="cajetin" and comp.get(family) is not None else comp.get(family,[]) if family!="cajetin" else []
            for row in rows: row.pop("z",None)
        return comp
    def normalize_ids(value,reverse,key=None):
        if key in IDENTITIES and isinstance(value,str): return reverse.get(value,value)
        if isinstance(value,dict): return {k:normalize_ids(v,reverse,k) for k,v in value.items()}
        if isinstance(value,list): return [normalize_ids(v,reverse) for v in value]
        return value
    def rows(comp):
        for family in FAMILIES:
            items=[comp[family]] if family=="cajetin" and comp.get(family) is not None else comp.get(family,[]) if family!="cajetin" else []
            for row in items: yield family,row
    def fingerprint(family,row,reverse):
        row=normalize_ids(row,reverse);assert "z" in row and type(row["z"]) in (int,float) and math.isfinite(row["z"]),"typed finite model z"
        z=row.pop("z")
        return (family,json.dumps(row,sort_keys=True,separators=(",",":")),z)
    def projection(state):
        state=copy.deepcopy(state)
        for k in ("selection_order","panel_owner","system_text"): state.pop(k)
        cats=[v["category"] for v in state["clipboard"]]
        assert len(cats)==len(set(cats)),"Classifier scope requires one copied model per concrete kind"
        state["clipboard"].sort(key=lambda v:v["category"])
        state["composition"]=strip_z(state["composition"])
        state["all_compositions"]=[strip_z(c) for c in state["all_compositions"]]
        return state
    for case in source["cases"]:
        expected=[v for v in source["states"] if v["case"]==case["name"]];actual=[v for v in native["states"] if v["case"]==case["name"]]
        comparator=Comparator(old_ids(expected[0]["all_compositions"]));z_registry={};texts=["",""]
        for i,(a,b) in enumerate(zip(expected,actual)):
            try:
                comparator.equal(projection(a),projection(b),"residual-models")
                for state in (a,b):
                    assert sorted(state["selection_order"],key=lambda o:(o["family"],o["index"]))==state["selection"],"selection iteration changed membership"
                action=case["actions"][i] if i<len(case["actions"]) else {"op":"save"}
                for j,state in enumerate((a,b)):
                    before=(expected if j==0 else actual)[max(i-1,0)]
                    if action["op"]=="clipboard" and action["key"] in ("C","X") and before["selection"]:
                        content=[v["record"].get("text","") for v in state["clipboard"] if v["category"] in (1,10) and v["record"].get("text")]
                        if content:texts[j]="\n".join(content)
                    assert state["system_text"]==texts[j],"system text does not follow actual copied sequence"
                if a["panel_owner"]!=b["panel_owner"]:
                    assert action["op"]=="clipboard" and action["key"]=="V","shown owner difference outside Paste"
                    for state in (a,b):
                        owner=state["panel_owner"];assert owner in state["selection"],"shown owner not pasted member"
                        z=snapshot(state["composition"],owner)["record"]["z"]
                        assert z==max(snapshot(state["composition"],o)["record"]["z"] for o in state["selection"]),"shown owner not first highest-z force-selected item"
                registration=i==0 or (action["op"]=="clipboard" and action["key"]=="V")
                for sheet,(ac,bc) in enumerate(zip(a["all_compositions"],b["all_compositions"])):
                    ar=list(rows(ac));br=list(rows(bc));assert len(ar)==len(br)
                    for (family,row),(bf,nrow) in zip(ar,br):
                        key=(sheet,*fingerprint(family,row,{}));native_key=(sheet,*fingerprint(bf,nrow,comparator.reverse))
                        assert key[:-1]==native_key[:-1],"whole model identity/field mismatch"
                        native_z=native_key[-1]
                        if i==0: assert key[-1]==native_z and (native_z!=0 or math.copysign(1.,key[-1])==math.copysign(1.,native_z)),"original z differs"
                        if registration and key not in z_registry:z_registry[key]=native_z
                        assert key in z_registry and z_registry[key]==native_z,"z differs from its validated creation/history record"
            except (AssertionError,KeyError,TypeError,ValueError,IndexError) as e:mismatches.append({"case":case["name"],"phase":a["phase"],"difference":repr(e.args)})
        try:
            a=next(v for v in original_readers if v["case"]==case["name"]);b=next(v for v in native_readers if v["case"]==case["name"])
            comparator.equal([strip_z(c) for c in a["compositions"]],[strip_z(c) for c in b["compositions"]],"residual-readers")
            for sheet,(ac,bc) in enumerate(zip(a["compositions"],b["compositions"])):
                for (family,row),(bf,nrow) in zip(rows(ac),rows(bc)):
                    key=(sheet,*fingerprint(family,row,{}));nkey=(sheet,*fingerprint(bf,nrow,comparator.reverse))
                    assert key[:-1]==nkey[:-1] and z_registry.get(key)==nkey[-1],"reader z/model differs from validated published record"
        except (AssertionError,KeyError,TypeError,ValueError,IndexError) as e:mismatches.append({"case":case["name"],"phase":"reader","difference":repr(e.args)})
    return {"ok":not mismatches,"states":68,"original_reader_pairs":6,"mismatches":mismatches,"scope":"Finite unique-kind clipboard traversal classifier. Every other model/type/array/history/member field remains literal; clipboard permutation, validated per-creation z mapping, highest-pasted-z shown owner and actual copied-text sequence are the only allowed residuals, with stable fresh identity bijections. Strict captured output parity remains independently failed."}

def classifier_negatives(source,native,original,readers):
    probes=[]
    press=next(i for i,v in enumerate(native["states"]) if v["case"]=="text-copy-cut-history" and v["phase"].endswith("-press"))
    undo=next(i for i,v in enumerate(native["states"]) if v["case"]=="text-copy-cut-history" and v["phase"]=="paste-undo")
    for name,index,row,key,value in [("non-key-model",press,0,"x_mm",999.),("original-z",0,0,"z",99.),("original-z-bool",0,0,"z",False),("non-key-pasted-z",undo,1,"z",999.)]:
        bad=copy.deepcopy(native)
        bad["states"][index]["composition"]["texts"][row][key]=value
        bad["states"][index]["all_compositions"][0]["texts"][row][key]=value
        try: rejected=not traversal_residuals(source,bad,original,readers)["ok"]
        except (AssertionError,KeyError,TypeError,ValueError,IndexError): rejected=True
        assert rejected,("classifier negative accepted",name)
        probes.append(name)
    return probes

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
        result["order_parametric_source_law"]=source_law(source,native,readers)
        history_diffs=[{"case":e["case"],"phase":e["phase"],"source":{"undo":e["undo"],"redo":e["redo"]},"native":{"undo":n["undo"],"redo":n["redo"]}} for e,n in zip(source["states"],native["states"]) if e["undo"]!=n["undo"] or e["redo"]!=n["redo"]]
        result["literal_all_state_history"]={"ok":not history_diffs,"states":68,"mismatches":history_diffs}
        try: result["traversal_residual_classification"]=traversal_residuals(source,native,original,readers)
        except Exception as e: result["traversal_residual_classification"]={"ok":False,"error":repr(e)}
        if result["traversal_residual_classification"]["ok"]:
            result["traversal_residual_classification"]["rejected_controls"]=classifier_negatives(source,native,original,readers)
        result.update(cases=6,states=68,original_reader_pairs=6,scope="Strict ordered source states and models; fresh UID/group spelling only is bijective. z/clipboard/selection order remain strict. Native session-reset persistence is not native window-close proof.",source_noops=["locked-hidden-copy: force_select acquires none; Copy/Paste are no-ops", "text-copy-cut-history: Cut after history-cleared selection is a no-op"],operative_cut="cross-sheet-bindings Text+Cota",source_callback_errors=len(source["callback_errors"]))
        if result["ok"]: result["rejected_controls"]=rejects(source,native,original,readers)
    except Exception as e: result["error"]=repr(e)
    a.comparison.parent.mkdir(parents=True,exist_ok=True);a.comparison.write_text(json.dumps(result,indent=2)+"\n")
    assert result.get("ok") and "error" not in result,result
if __name__=="__main__": main()
