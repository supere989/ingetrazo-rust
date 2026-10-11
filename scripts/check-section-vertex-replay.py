#!/usr/bin/env python3
"""Bounded paired vertex replay admission, not production VS instruction identity."""
import copy,hashlib,json,math,struct,sys,zipfile
from pathlib import Path
SOURCE_SHA="08ceb200be83cb4e2099ab51a211f248818c94b01818a05d4e81c7a6bd16012c"
FRAME_SHA="9fabf645e1f3518902a2cfebbbae600663d48e8caf3980391d44a12313e88e55"
def floats(bits):
 assert isinstance(bits,list) and all(type(v)is int and 0<=v<2**32 for v in bits)
 result=[struct.unpack('<f',struct.pack('<I',v))[0]for v in bits]
 assert all(math.isfinite(v)for v in result)
 return result
def flatten(a):return [v for row in a for v in row]
def validate(n,s):
 o=s['observation'];assert s['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9' and s['ordinary_and_observed_fullframe_exact'] is True
 assert n['separately_compiled_vertex_replay'] is True and n['production_vs_instruction_identity_claim'] is False
 assert n['compiler_options']=='None (same as production)' and n['completed'] is True and n['output_shared'] is True and n['output_bytes']==3840 and n['vertex_stride']==48
 assert o['separately_linked_original_glsl'] is True and o['production_instruction_identity_claim'] is False
 assert o['varyings']==[['gl_Position',1,0x8B52],['gl_ClipDistance',1,0x1406],['v_world',1,0x8B51]]
 assert o['model_attribute_arrays_enabled']==[0,0,0,0]
 assert flatten(n['model_bits'])==flatten(o['generic_model_attribute_bits'])
 assert flatten(n['camera_bits'])==o['camera_bits'];floats(flatten(n['camera_bits']))
 assert len(n['indices'])==60 and all(type(i)is int and 0<=i<60 for i in n['indices'])
 assert len(n['uploads'])==len(n['outputs'])==len(o['upload_bits'])==len(o['outputs_bits'])==60
 assert [v['id']for v in n['uploads']]==[v['id']for v in n['outputs']]==list(range(60))
 plane_native=floats(n['section_plane_bits']);plane_source=floats(o['section_plane_bits']);assert plane_native==plane_source
 differences=[]
 for draw_id,index in enumerate(n['indices']):
  upload=n['uploads'][index];out=n['outputs'][index];source_upload=o['upload_bits'][draw_id];source_out=o['outputs_bits'][draw_id]
  assert len(upload['position_bits'])==3 and upload['position_bits']==source_upload[:3]
  floats(source_upload);floats(upload['position_bits']);floats(source_out)
  assert len(source_out)==8
  for key in ['world_bits','gl_clip_bits','metal_clip_bits']:assert len(out[key])==4;floats(out[key])
  floats([out['clip_distance_bits']])
  # Report compiled arithmetic differences, never turn equality into instruction-identity proof.
  differences.append({'draw_vertex':draw_id,'source_face':draw_id//6,'triangle':draw_id%6//3,'native_vertex_id':index,'source_gl_clip_bits':source_out[:4],'native_gl_clip_bits':out['gl_clip_bits'],'source_clip_distance_bits':source_out[4],'native_clip_distance_bits':out['clip_distance_bits'],'source_world_bits':source_out[5:8],'native_world_bits':out['world_bits'],'gl_clip_equal_bits':source_out[:4]==out['gl_clip_bits'],'clip_distance_equal_bits':source_out[4]==out['clip_distance_bits'],'world_xyz_equal_bits':source_out[5:8]==out['world_bits'][:3]})
 return {'bounded_replay_admission_pass':True,'production_vs_instruction_identity_claim':False,'release_parity_claim':False,'actual_input_positions_indices_camera_model_matched':True,'plane_mathematically_equal':True,'plane_exact_bits_equal':n['section_plane_bits']==o['section_plane_bits'],'plane_bit_differences':[{'component':i,'source':a,'native':b,'source_float':plane_source[i],'native_float':plane_native[i]}for i,(a,b)in enumerate(zip(o['section_plane_bits'],n['section_plane_bits']))if a!=b],'gl_clip_different_vertices':sum(not d['gl_clip_equal_bits']for d in differences),'clip_distance_different_vertices':sum(not d['clip_distance_equal_bits']for d in differences),'world_xyz_different_vertices':sum(not d['world_xyz_equal_bits']for d in differences),'vertices':differences,'limitation':'Separate void Metal replay and separately linked GL transform feedback can change compiler association; matching outputs do not prove production rasterizing VS instruction identity.'}
def main():
 root=Path(sys.argv[1]);archive=Path(sys.argv[2]);assert hashlib.sha256(archive.read_bytes()).hexdigest()==SOURCE_SHA
 with zipfile.ZipFile(archive)as z:s=json.loads(z.read('report.json'))
 for filename in ['closed-iso-ordinary.rgba','closed-iso-observed.rgba']:
  data=(root/filename).read_bytes();assert len(data)==1920000 and hashlib.sha256(data).hexdigest()==FRAME_SHA
 n=json.loads((root/'vertex-replay.json').read_text());result=validate(n,s)
 # Same validation function must reject malformed/changed input, incomplete and nonfinite output.
 controls={}
 mutations={'position':lambda x:x['uploads'][0]['position_bits'].__setitem__(0,1065353216),'missing_output':lambda x:x['outputs'].pop(),'camera':lambda x:x['camera_bits'][0].__setitem__(0,0),'model':lambda x:x['model_bits'][0].__setitem__(0,0),'uncompleted':lambda x:x.__setitem__('completed',False),'nonfinite_output':lambda x:x['outputs'][0]['gl_clip_bits'].__setitem__(0,0x7fc00000)}
 for name,mutate in mutations.items():
  bad=copy.deepcopy(n);mutate(bad)
  try:validate(bad,s)
  except AssertionError:controls[name]='rejected'
  else:raise AssertionError('Negative accepted: '+name)
 # Signedzero is a reportable representational difference, not a mathematical plane refusal.
 zero=copy.deepcopy(n);zero['section_plane_bits'][0]^=0x80000000;accepted=validate(zero,s);assert accepted['plane_mathematically_equal'] and accepted['plane_bit_differences'];controls['signedzero_plane']='accepted and reported'
 result['negative_controls']=controls;print(json.dumps(result,indent=2))
if __name__=='__main__':main()
