#!/usr/bin/env python3
"""Independent IfcOpenShell IFC4 reader/EXPRESS gate; no native build or writer reuse."""
import argparse,hashlib,json,pathlib,sys,copy
import ifcopenshell,ifcopenshell.validate
import _pytest  # IfcOpenShell EXPRESS executor dependency.
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
CLASSES=['IfcWall','IfcSlab','IfcColumn','IfcBeam','IfcFooting','IfcRoof','IfcStair','IfcRamp','IfcDoor','IfcWindow','IfcRailing','IfcCovering','IfcPile','IfcMember','IfcSpace']
NAMES=CLASSES+['wall-hole','rotated-door','transformed-nested','unknown-class','untagged']
METRIC_KEYS=['class','name','area','volume','quantities']
def validate_case(case,record,model):
 assert record['name']==case['name']
 assert len(record['objects'])==len(case['objects'])
 for source,native in zip(case['objects'],record['objects']):
  assert {k:source[k]for k in METRIC_KEYS}=={k:native[k]for k in METRIC_KEYS},'native metric drift'
 if not case['objects']:
  assert model is None and record['export_count'] is None and record['export_error'],'untagged export claim'
  return {'name':case['name'],'objects':0}
 assert model is not None and model.schema=='IFC4'
 assert record['export_count']==len(case['objects']) and record['export_error'] is None
 assert model.header.file_name.name=="José's Project"
 products=[x for x in model.by_type('IfcProduct') if x.Representation]
 assert len(products)==len(case['objects']),'IFC product count'
 for obj,product in zip(case['objects'],products):
  assert product.is_a()==(obj['class'] if obj['class'] in CLASSES else 'IfcBuildingElementProxy'),'IFC class'
  assert product.Name==(obj['name'] or obj['class']),'IFC name'
  rep=product.Representation.Representations[0];assert rep.RepresentationIdentifier=='Body'
  assert len(rep.Items)==1;item=rep.Items[0]
  closed=obj['volume'] is not None
  assert item.is_a()==('IfcFacetedBrep' if closed else 'IfcShellBasedSurfaceModel'),'open/closed representation'
  assert rep.RepresentationType==('Brep' if closed else 'SurfaceModel')
  shell=item.Outer if closed else item.SbsmBoundary[0]
  assert shell.is_a()==('IfcClosedShell' if closed else 'IfcOpenShell')
  rings=[]
  for face in shell.CfsFaces:
   assert face.Bounds[0].is_a('IfcFaceOuterBound')
   assert all(b.Orientation is True for b in face.Bounds)
   rings.append([[list(v.Coordinates)for v in b.Bound.Polygon]for b in face.Bounds])
  assert rings==obj['rings'],'world polygon/hole coordinate drift'
  sets=[r.RelatingPropertyDefinition for r in product.IsDefinedBy if r.is_a('IfcRelDefinesByProperties') and r.RelatingPropertyDefinition.is_a('IfcElementQuantity')]
  assert len(sets)==int(bool(obj['quantities']['entries']))
  if sets:
   eq=sets[0];assert eq.Name==obj['quantities']['set']
   expected=obj['quantities']['entries'];assert len(eq.Quantities)==len(expected)
   for value,q in zip(expected,eq.Quantities):
    assert q.is_a()=={'length':'IfcQuantityLength','area':'IfcQuantityArea','volume':'IfcQuantityVolume'}[value['kind']]
    assert q.Name==value['name'] and q[3]==value['value'],'quantity drift'
  if product.is_a() in ['IfcDoor','IfcWindow']:
   values={q['name']:q['value']for q in obj['quantities']['entries']}
   assert product.OverallHeight==values.get('Height') and product.OverallWidth==values.get('Width')
 logger=ifcopenshell.validate.json_logger();ifcopenshell.validate.validate(model,logger,express_rules=True)
 assert not logger.statements,json.dumps(logger.statements,default=str)
 return {'name':case['name'],'objects':len(products),'express_issues':0}
def main():
 parser=argparse.ArgumentParser();parser.add_argument('original',type=pathlib.Path);parser.add_argument('native',type=pathlib.Path);parser.add_argument('--output',type=pathlib.Path);args=parser.parse_args()
 original=json.loads((args.original/'original.json').read_text());native=json.loads((args.native/'native.json').read_text())
 assert original['source_pin']==PIN and [c['name']for c in original['cases']]==NAMES and [c['name']for c in native]==NAMES
 assert {p.stem for p in args.native.glob('*.ifc')}==set(NAMES)-{'untagged'}
 results=[];failures=[]
 for case,record in zip(original['cases'],native):
  path=args.native/(case['name']+'.ifc');model=ifcopenshell.open(str(path)) if path.exists() else None
  try:results.append(validate_case(case,record,model))
  except Exception as error:failures.append({'name':case['name'],'error':str(error)})
 report={'passed':not failures,'validator_version':ifcopenshell.version,'cases':results,'failures':failures,'source_pin':PIN,'ifc_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest()for p in args.native.glob('*.ifc')}}
 if args.output:args.output.write_text(json.dumps(report,indent=2)+'\n')
 print(json.dumps(report,indent=2));return 0 if report['passed'] else 1
if __name__=='__main__':raise SystemExit(main())
