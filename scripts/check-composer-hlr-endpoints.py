#!/usr/bin/env python3
"""Compare frozen original/native HLR endpoints with class identity, not counts."""
import argparse,json,math,copy
from pathlib import Path
def matches(a,b,tol=2e-5):
 if len(a["world"])!=len(b["world"]) or len(a["kinds"])!=len(a["world"]) or len(b["kinds"])!=len(b["world"]):return False
 remaining=list(zip(b["world"],b["kinds"]))
 def point(p,q):return all(abs(x-y)<=tol for x,y in zip(p,q))
 for segment,kind in zip(a["world"],a["kinds"]):
  for i,(candidate,other) in enumerate(remaining):
   if kind==other and ((point(segment[0],candidate[0])and point(segment[1],candidate[1]))or(point(segment[0],candidate[1])and point(segment[1],candidate[0]))):
    remaining.pop(i);break
  else:return False
 return not remaining
def inside(point,ring):
 x,y=point;result=False
 for a,b in zip(ring,ring[1:]+ring[:1]):
  if (a[1]>y)!=(b[1]>y) and x<(b[0]-a[0])*(y-a[1])/(b[1]-a[1])+a[0]:result=not result
 return result
def caps_match(loops,triangles):
 if not loops:return not triangles
 if not triangles:return False
 def area(r):return abs(sum(a[0]*b[1]-a[1]*b[0]for a,b in zip(r,r[1:]+r[:1])))/2
 # Nesting depth defines the original even-odd filled area.
 expected=0
 for i,r in enumerate(loops):
  depth=sum(inside(r[0],other)for j,other in enumerate(loops)if i!=j)
  expected+=area(r)*(-1 if depth%2 else 1)
 actual=sum(area(t)for t in triangles)
 if abs(expected-actual)>2e-5:return False
 points=[p for ring in loops for p in ring]+[p for t in triangles for p in t]
 lo=[min(p[k]for p in points)-.037 for k in [0,1]];hi=[max(p[k]for p in points)+.053 for k in [0,1]]
 for i in range(83):
  for j in range(79):
   point=[lo[0]+(hi[0]-lo[0])*(i+.371)/83,lo[1]+(hi[1]-lo[1])*(j+.619)/79]
   original=sum(inside(point,r)for r in loops)%2==1
   native=any(inside(point,t)for t in triangles)
   if original!=native:return False
 return True
def main():
 p=argparse.ArgumentParser();p.add_argument("corpus",type=Path);p.add_argument("--native-revision",required=True);p.add_argument("--require-caps",action="store_true");p.add_argument("--output",type=Path,default=Path("docs/audits/composer-hlr-endpoint-evidence.json"));a=p.parse_args();rows=[]
 for source in sorted(a.corpus.glob("*.reference.json")):
  ref=json.loads(source.read_text());name=ref["name"]
  for hidden in ["false","true"]:
   native=json.loads((a.corpus/(name+"."+hidden+".rust.json")).read_text());original=ref["drawings"][hidden];same=matches(original,native)
   if a.require_caps:same=same and caps_match(original["loops"],native["cap_triangles"])
   rows.append({"case":name,"hidden":hidden,"match":same,"original":original,"native":native,"original_revision":ref["source_commit"]})
 assert len(rows)==16,"Missing fixture: eight cases and both visibility settings required"
 baseline=rows[0]["original"];controls=[]
 for name in ["moved-endpoint","reclassified-line","deleted-segment"]:
  changed=copy.deepcopy(baseline)
  if name=="moved-endpoint":changed["world"][0][0][0]+=.1
  elif name=="reclassified-line":changed["kinds"][0]+=10
  else:changed["world"].pop();changed["kinds"].pop()
  assert not matches(baseline,changed);controls.append({"case":name,"detected":True})
 if a.require_caps:
  original=next(r["original"]for r in rows if r["case"]=="hollow_cap");native=next(r["native"]for r in rows if r["case"]=="hollow_cap");changed=[[[x+.3,y]for x,y in t]for t in native["cap_triangles"]];assert not caps_match(original["loops"],changed);controls.append({"case":"shifted-cap-same-area","detected":True})
 report={"native_revision":a.native_revision,"passed":all(r["match"]for r in rows),"endpoint_tolerance":2e-5,"caps_required":a.require_caps,"cap_area_tolerance":2e-5,"cap_coverage_grid":[83,79],"negative_controls":controls,"cases":rows,"scope":"Frozen source-worker actual original and native outputs independently compared as endpoint/class multisets; eight orthographic fixtures both hidden modes. Fresh executable provenance, GUI rendering, line widths, projected segments and closed loops require separate acceptance."};a.output.write_text(json.dumps(report,indent=2)+"\n");print(json.dumps([{k:r[k]for k in ["case","hidden","match"]}for r in rows],indent=2));assert report["passed"]
if __name__=="__main__":main()
