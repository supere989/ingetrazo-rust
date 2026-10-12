"""SKP source App state: directed geometry, local axes and authored sharing."""
from exchange_import_observations import mesh_state,texture,vec
from PySide6.QtGui import QMatrix4x4 as M

def scene_state(s):
 owners={}
 def owner(mesh):
  if not mesh.vertices:return None
  return owners.setdefault(id(mesh),len(owners))
 def attrs_texture(a):
  a=dict(a)
  if isinstance(a.get('texture'),dict):a['texture']=texture(a['texture'])
  if isinstance(a.get('back'),dict):a['back']=attrs_texture(a['back'])
  return a
 def observed_mesh(mesh,m=None):
  out=mesh_state(mesh,m)
  # mesh_state's front resource observation is shared; back paint remains owned.
  for face in out['faces']:
   if isinstance(face['attrs'].get('back'),dict):face['attrs']['back']=attrs_texture(face['attrs']['back'])
  out['faces'].sort(key=lambda f:__import__('json').dumps(f,sort_keys=True))
  return out
 def group(g,parent):
  local=g.xform if g.xform is not None else M();world=parent*local
  return {'name':g.name,'layer':g.layer,'local_matrix':[float(x)for x in local.data()],
   'axes':[float(x)for x in (g.axes if g.axes is not None else M()).data()],
   'component':bool(g.component),'billboard':g.billboard,'mesh_owner':owner(g.mesh),
   'local_mesh':observed_mesh(g.mesh),'world_mesh':observed_mesh(g.mesh,world),
   'children':[group(c,world)for c in g.children]}
 materials={}
 for name,mat in s.materials.items():
  recipe=mat.to_dict()
  if isinstance(recipe.get('texture'),dict):recipe['texture']=texture(recipe['texture'])
  materials[name]=recipe
 return {'active':observed_mesh(s.mesh),'loose':observed_mesh(s.loose_mesh),
  'groups':[group(g,M())for g in s.groups], 'materials':materials,
  'layers':[l.to_dict()for l in s.layers], 'views':[v.to_dict()for v in s.saved_views],
  'dimensions':[[vec(d.a),vec(d.b),vec(d.offset),d.layer,d.text,d.axis,d.anchor_a is not None,d.anchor_b is not None]for d in s.dimensions],
  'texts':[t.to_dict()for t in s.text_labels], 'back_face_color':list(s.back_face_color)if s.back_face_color is not None else None,
  'units':s.units}
