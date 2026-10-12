"""Borrowed source record observations; no source mutation or expected geometry construction."""
def xyz(p):return[p.x(),p.y(),p.z()]
def plain(v):
 if hasattr(v,'tolist'):return plain(v.tolist())
 if isinstance(v,(tuple,list)):return[plain(x)for x in v]
 if isinstance(v,dict):return{k:plain(x)for k,x in v.items()}
 return v

def geometry(scene):
 mesh=scene.mesh;ids={v:i for i,v in enumerate(mesh.vertices)}
 return dict(vertices=[xyz(v.position)for v in mesh.vertices],edges=[dict(vertices=[ids[e.v0],ids[e.v1]],soft=e.soft,curve=e.curve,layer=e.layer,hidden=getattr(e,'hidden',False))for e in mesh.edges],faces=[dict(loop=[ids[v]for v in f.loop],holes=[[ids[v]for v in h]for h in f.hole_loops],interior=f.interior,attrs=f.attrs)for f in mesh.faces])
def annotations(scene):return dict(dimensions=[dict(a=xyz(d.a),b=xyz(d.b),offset=xyz(d.offset),axis=d.axis,layer=d.layer,text=d.text,value=d.value(),anchor_a=d.anchor_a is not None,anchor_b=d.anchor_b is not None)for d in scene.dimensions],text_labels=[l.to_dict()for l in scene.text_labels],geo_paths=[p.to_dict()for p in scene.geo_paths],section_planes=[p.to_dict()for p in scene.section_planes],dimension_style=scene.dimension_style)
def viewport_state(vp):
 from core.saved_views import georef_objects
 c=vp.camera;s=vp.scene
 return dict(active_section_uid=None if s.active_section() is None else s.active_section().uid,geographic_visibility={key:obj.visible for key,obj in georef_objects(s)if obj is not None and hasattr(obj,"visible")},camera=dict(target=xyz(c.target),up=xyz(c.up),distance=c.distance,yaw=c.yaw,pitch=c.pitch,fov=c.fov_deg,perspective=c.perspective,aspect=c.aspect),layers=[l.to_dict()for l in s.layers],section=[p.to_dict()for p in s.section_planes],show_section_planes=s.show_section_planes,show_section_cuts=s.show_section_cuts,display_style=None if s.display_style is None else s.display_style.to_dict(),shadows=s.shadows.to_dict())
