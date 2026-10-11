#!/usr/bin/env python3
"""Strict full RGBA comparisons for prototype and untouched reference; all failures retained."""
import argparse,collections,hashlib,importlib.util,json
from pathlib import Path
spec=importlib.util.spec_from_file_location('strict_section',Path(__file__).with_name('check-metal-section-fill.py'));strict=importlib.util.module_from_spec(spec);spec.loader.exec_module(strict)
def validate(root,reference):
 report=json.loads((root/'report.json').read_text());assert [r['case']for r in report['cases']]==strict.NAMES
 rows=[]
 for row in report['cases']:
  name=row['case'];assert row['warm_frames']==3
  for counter in ['target_allocations','geometry_uploads','section_fill_uploads','section_fill_upload_bytes','section_contour_pipeline_builds','resolve_pipeline_builds']:assert row[counter+'_before']==row[counter+'_after']
  assert row['resolve_pipeline_builds_before']==1
  assert row['contours_enabled'] is True and row['fill_off_contours_enabled'] is True and row['cuts_off_contours_enabled'] is False
  diagnostic=row['resolve_prototype'];assert diagnostic['supports_family_apple4'] is (diagnostic['path']=='same-pass-tile')
  assert diagnostic['extra_output_images']==0 and diagnostic['srgb_linearization'] is False and diagnostic['fast_math'] is False
  source=strict.image(reference/(name+'.png'));native=(root/(name+'.rgba')).read_bytes();candidate=(root/(name+'-candidate.rgba')).read_bytes();disabled=(root/(name+'-candidate-disabled.rgba')).read_bytes();assert len(source)==len(native)==len(candidate)==len(disabled)==800*600*4
  cap=lambda p:p[0]>180 and p[2]>180 and p[1]<80
  counts={'reference_rgba_mismatch_pixels':0,'candidate_rgba_mismatch_pixels':0,'reference_cap_mask_mismatch_pixels':0,'candidate_cap_mask_mismatch_pixels':0,'candidate_changed_pixels_from_reference':0,'newly_wrong_pixels':0,'corrected_pixels':0};pairs=collections.Counter()
  for i in range(0,len(source),4):
   s,n,c,d=source[i:i+4],native[i:i+4],candidate[i:i+4],disabled[i:i+4]
   assert not cap(d),(name,'fill-off cap')
   counts['reference_rgba_mismatch_pixels']+=s!=n;counts['candidate_rgba_mismatch_pixels']+=s!=c;counts['reference_cap_mask_mismatch_pixels']+=cap(s)!=cap(n);counts['candidate_cap_mask_mismatch_pixels']+=cap(s)!=cap(c);counts['candidate_changed_pixels_from_reference']+=n!=c;counts['newly_wrong_pixels']+=s==n and s!=c;counts['corrected_pixels']+=s!=n and s==c
   if s!=c:pairs[(tuple(s),tuple(c))]+=1
  assert counts['candidate_changed_pixels_from_reference']==row['candidate_changed_pixels_from_reference']
  rows.append({'case':name,**counts,'reference_sha256':hashlib.sha256(native).hexdigest(),'candidate_sha256':hashlib.sha256(candidate).hexdigest(),'candidate_mismatch_color_pairs':[{'source':a,'candidate':b,'pixels':v}for (a,b),v in pairs.most_common()],'resolve':diagnostic})
 return {'cases':rows,'passed':all(r['candidate_rgba_mismatch_pixels']==0 for r in rows),'reference_passed':all(r['reference_rgba_mismatch_pixels']==0 for r in rows),'scope':'All ten full RGBA images; zero tolerance. Unchanged reference and opt-in candidate measured independently. Remaining iso geometry/color failures are not normalized.'}
def main():
 p=argparse.ArgumentParser();p.add_argument('native',type=Path);p.add_argument('--reference',type=Path,default=Path('docs/audits/section-original-gl'));p.add_argument('--output',type=Path);a=p.parse_args();r=validate(a.native,a.reference);text=json.dumps(r,indent=2)+'\n';print(text)
 if a.output:a.output.write_text(text)
 raise SystemExit(0 if r['passed']else 1)
if __name__=='__main__':main()
