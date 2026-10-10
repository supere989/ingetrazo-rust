#!/usr/bin/env python3
"""Original GL/native Vulkan stencil coverage: existing 5% positive, exact zero gates."""
import argparse,json,subprocess,hashlib
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('original');p.add_argument('native_root');p.add_argument('wrong_winding_image');a=p.parse_args()
def count(path):
 size=subprocess.check_output(['identify','-format','%w,%h',str(path)],text=True)
 assert size=='800,600',size
 pixels=subprocess.check_output(['convert',str(path),'-depth','8','rgb:-'])
 return sum(pixels[i]>180 and pixels[i+2]>180 and pixels[i+1]<80 for i in range(0,len(pixels),3))
def acceptable(reference,native):return native==0 if reference==0 else abs(native-reference)/reference<=.05
rows=[]
reference_rows=json.loads(Path(a.original).read_text())['cases']
expected=["closed","shared-slab","shared-slab-reversed","transparent","duplicate","coplanar-backface","closed-iso","shared-slab-iso","shared-slab-reversed-iso","closed-front"]
assert [row['case'] for row in reference_rows]==expected,"Missing/reordered original stencil controls"
for row in reference_rows:
 path=Path(a.native_root)/('msaa-'+row['case'])/'resize-800x600.png'
 native=count(path);reference=row['magenta_fill_pixels']
 assert acceptable(reference,native),(row['case'],reference,native)
 rows.append({'case':row['case'],'original':reference,'native':native,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()})
wrong=count(a.wrong_winding_image)
assert not acceptable(rows[0]['original'],wrong),'Actual wrong-winding positive failure was not rejected'
assert not acceptable(0,rows[0]['native']),'Physical closed-cap positive incorrectly accepted for shared partition zero case'
print(json.dumps({'cases':rows,'negative_controls':{'actual_wrong_winding_closed':wrong,'physical_fill_in_zero_case_rejected':True},'scope':'Linux software-native Vulkan bounded stencil, exact zero and established 5% positive tolerance; not full rendering parity'},indent=2))
