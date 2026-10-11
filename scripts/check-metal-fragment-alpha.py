"""Validate all three ordinary fragment alpha routes against pinned shader law.
This does not certify Metal blending, incoming RGB conversion or resolve pixels.
"""
import json,pathlib,re
root=pathlib.Path(__file__).resolve().parents[1]
def validate(shader):
 body=re.search(r'float\s+source_fragment_alpha\s*\(float\s+alpha\)\s*\{([^}]*)\}',shader)
 if body is None or re.sub(r'\s','',body.group(1))!='returnclamp(alpha,0.,1.);':raise ValueError('raw alpha body')
 for value in ['paint.opacity','texel.a*paint.opacity']:
  expected=2 if value=='paint.opacity' else 1
  if shader.count('source_fragment_alpha('+value+')')!=expected:raise ValueError('missing alpha route')
 if 'source_blend_alpha' in shader:raise ValueError('old encoded law')
shader=(root/'crates/ingetrazo-metal/src/shaders/world.metal').read_text();validate(shader)
source=(root/'ingetrazo/resources/shaders/basic.frag').read_text()
assert 'vec4(v_color, u_opacity)' in source and 'texel.a * u_opacity' in source
negatives=[shader.replace('return clamp(alpha,0.,1.);','return rint(clamp(alpha,0.,1.)*255.)/255.;'),shader.replace('source_fragment_alpha(texel.a*paint.opacity)','texel.a'),shader.replace('source_fragment_alpha(paint.opacity)','0.5',1)]
for candidate in negatives:
 try:validate(candidate)
 except ValueError:pass
 else:raise AssertionError('mutated shader accepted')
mesa=json.loads((root/'docs/audits/metal-world-scene/material-alpha-source/review.json').read_text())
assert len(mesa['cases'])==6 and sum(c['unencoded_rejected']for c in mesa['cases'])==5
print(json.dumps({'raw_alpha_routes':3,'rejected_shader_mutations':len(negatives),'retained_mesa_cases':6,'mesa_raw_prediction_disagreements':5,'scope':'static source fragment law only; prior Mesa pixel oracle remains unchanged and may reject actual native frames; incoming RGB/attachment/resolve precision unproven'},indent=2))
