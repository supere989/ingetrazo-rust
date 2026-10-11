#!/usr/bin/env python3
"""Pinned original writer/reader with every source scene field present together."""
import ast,hashlib,json,sys,zipfile
from pathlib import Path
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M,QImage,QColor
from core.scene import Scene
from core.group import Group
from core.mesh import Mesh
from core.layers import Layer
from core.materials import Material
from core.saved_views import SavedView
from core.composition import Composicion
from core.dimension import Dimension
from core.guide import Guide
from core.textlabel import TextLabel
from core.section import SectionPlane
from core.style import Style
from core.sun import ShadowSettings
from georef.datum import SceneDatum
from georef.geopath import GeoPath
from georef.points import GeoPoint
from georef.tiles import TileLayer
from georef.photomesh import PhotoMesh,PhotoMaterial,ODMAnchor
from formats.igz import save_scene,load_into
import numpy as np
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
CASES=['no-op','loose-edit','rename-parent','move-child','remove-copy','paint']
app=QApplication([])
def document(path):
    with zipfile.ZipFile(path) as z:return json.loads(z.read('document.json'))
def norm(value):
    if isinstance(value,list):return [norm(v)for v in value]
    if isinstance(value,dict):
        out={k:norm(v)for k,v in value.items()}
        # Runtime absolute texture paths are caches, not authored identity.
        for key in ('path','base'):
            v=out.get(key)
            if isinstance(v,str) and Path(v).is_file():out[key]={'sha256':hashlib.sha256(Path(v).read_bytes()).hexdigest()}
        return out
    return value
def observation(path, staging):
    scene=Scene();load_into(scene,path)
    # Re-serialize through the actual source writer to cover every record
    # type without a second handwritten schema inventory.
    save_scene(scene,staging)
    raw=document(staging)['scene']
    # Canonicalize packed resource references by bytes, independently per file.
    with zipfile.ZipFile(staging) as z:
        def assets(v):
            if isinstance(v,dict):
                out={k:assets(x)for k,x in v.items()}
                for key in ('embed','geometry','atlas'):
                    if isinstance(out.get(key),str) and out[key] in z.namelist():out[key]={'sha256':hashlib.sha256(z.read(out[key])).hexdigest()}
                if 'embed' in out:out.pop('path',None)
                return out
            if isinstance(v,list):return [assets(x)for x in v]
            return v
        raw=assets(raw)
    return norm(raw)
def build(root):
    root.mkdir(parents=True,exist_ok=True)
    atlas=QImage(4,4,QImage.Format_RGB32);atlas.fill(QColor(96,128,160))
    texture=root/'Árbol 日本 😀.png';atlas.save(str(texture))
    s=Scene()
    f=s.mesh.add_face([V(0,0,0),V(4,0,0),V(4,4,0),V(0,4,0)])
    f.attrs.update({'color':[.2,.4,.6],'opacity':.8,'mat':'Mármol 日本','texture':{'path':str(texture),'sw':1.25,'sh':2.5,'rot':17.},'back':{'color':[.8,.3,.2],'opacity':.5},'layer':'Layer A'})
    s.mesh.edges[0].soft=True;s.mesh.edges[1].hidden=True;s.mesh.edges[2].curve=7;s.mesh.edges[3].layer='Layer A'
    leaf_mesh=Mesh();leaf_mesh.add_face([V(0,0,0),V(1,0,0),V(0,1,0)])
    leaves=[]
    for i in range(2):
        g=Group(leaf_mesh,name=f'Letter {i}');g.xform=M();g.xform.translate(i*2.,0,0);g.uid=f'leaf-{i:011d}';g.component=True;leaves.append(g)
    parent=Group(name='Container Ω');parent.xform=M();parent.xform.translate(5,2,0);parent.adopt(leaves);parent.component=False;parent.uid='parent-000000001'
    parent.axes=M();parent.axes.translate(.25,.5,.75)
    parent.layer='Layer A';parent.hidden=True;parent.ifc={'class':'IfcBuildingElementProxy','name':'Fixture','properties':{'mark':'P-1'}}
    parent.billboard='mesh';parent.material={'color':[.7,.6,.5],'opacity':.75};parent.ext={'extension':['Ω',{'enabled':True}]}
    parent.text3d={'text':'AB','font':'DejaVu Sans','bold':True,'italic':False,'height':.25,'thickness':.05}
    parent.exploded={'factor':.5,'custom':'retained'};parent.explode_offset=[.1,.2,.3]
    copy_group=Group(leaf_mesh,name='Shared copy');copy_group.xform=M();copy_group.xform.translate(-3,0,0);copy_group.uid='copy-00000000001';copy_group.component=True
    s.groups=[parent,copy_group]
    s.layers=[Layer('Layer 0'),Layer('Layer A',visible=False,locked=True)]
    s.materials={'Mármol 日本':Material('Mármol 日本',color=(.2,.4,.6),texture={'path':str(texture),'sw':1.25,'sh':2.5},opacity=.8,finish='glossy')}
    s.dimensions=[Dimension(V(0,0,0),V(4,0,0),V(0,1,0),layer='Layer A',text='four',axis='x')]
    s.dimension_style={'ends':'arrow','color':'#123456','font_px':17,'precision':4}
    s.camera_home={'target':[1.,2.,3.],'distance':12.5,'yaw':.25,'pitch':.5,'fov_deg':55.,'perspective':False,'two_point':True}
    s.units={'length':'cm','precision':4};s.plugin_data={'plugin.Ω':{'version':3,'data':[1,False,None,'日本']}};s.custom_scales=[12.5,33.,250.];s.back_face_color=(.15,.25,.35)
    s.section_planes=[SectionPlane(V(1,0,0),V(1,0,0),'Cut Ω','B',True,77)];s.show_section_planes=False;s.show_section_cuts=False;s.show_hidden_objects=True;s.show_hidden_geometry=True
    s.display_style=Style(name='Fixture',face_mode='monochrome',sky=False,profiles=False,back_edges=True)
    s.shadows=ShadowSettings(enabled=True,month=9,day=19,hour=10,minute=30,darkness=.65,utc_offset=-7)
    s.georef=SceneDatum(-16.39,-71.53,2345.,17.)
    s.tile_layer=TileLayer.from_dict({'source':'custom','zoom':15,'radius_m':300.,'visible':False,'opacity':.375,'patches':[[1.,2.,30.,40.]],'url_template':'https://example.invalid/{z}/{x}/{y}.png','name':'Fixture offline','attribution':'Fixture'})
    s.geo_paths=[GeoPath([V(0,0,0),V(3,0,1),V(3,4,2)],True,'Survey Ω','flat')]
    s.geo_points=[GeoPoint(V(1,2,3),'P-01','survey point')]
    s.text_labels=[TextLabel(V(1,1,0),V(2,2,1),'Label 日本','Layer A')]
    s.guides=[Guide(V(1,0,0),V(0,1,0),V(0,0,0)),Guide(V(2,3,4))]
    from core.image_plane import ImagePlane
    s.image_planes=[ImagePlane(str(texture),V(0,0,-1),V(4,0,0),V(0,4,0),name='Plan Ω',opacity=.375)]
    s.saved_views=[SavedView('View Ω',target=(1,2,3),distance=12.5,yaw=.25,pitch=.5,perspective=False,two_point=True,hidden_layers=['Layer A'],style=s.display_style.to_dict(),section=s.section_planes[0].to_dict(),georef=s.georef.to_dict(),shadows=s.shadows.to_dict(),hidden_objects=[parent.uid],hidden_shown={'objects':True,'geometry':True})]
    s.compositions=[Composicion(name='Sheet 日本',paper='A3',landscape=False,margin_mm=12.,border=True,guides_v=[25.,75.],guides_h=[33.])]
    photo=PhotoMesh(np.array([[0.,0.,0.],[1.,0.,0.],[0.,1.,.25]],dtype=np.float32),np.array([[0.,0.],[1.,0.],[0.,1.]],dtype=np.float32),np.array([[0,1,2]],dtype=np.uint32),[PhotoMaterial('Atlas',None,0,1)],Path('survey/original.obj'),ODMAnchor(717623.,8278698.,18,False))
    photo.images[0]=atlas;photo.visible=False;photo.layer='Layer A';s.photo_mesh=photo
    source=root/'all-fields.igz';save_scene(s,source)
    return source
def edit(s,kind):
    if kind=='loose-edit':s.mesh.add_edge(V(12,0,0),V(13,1,0));s.mesh.add_face([V(10,0,0),V(11,0,0),V(10,1,0)])
    elif kind=='rename-parent':s.groups[0].name='Renamed 日本'
    elif kind=='move-child':s.groups[0].children[0].xform.translate(.125,.25,.5)
    elif kind=='remove-copy':s.groups.pop()
    elif kind=='paint':s.mesh.faces[0].attrs['color']=[.125,.25,.75];s.mesh.faces[0].attrs['back']['opacity']=.625

def schema_keys():
    tree=ast.parse(Path('/work/ingetrazo/formats/igz.py').read_text())
    save=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='save_scene')
    keys=set()
    for n in ast.walk(save):
        if isinstance(n,ast.Subscript) and isinstance(n.value,ast.Name) and n.value.id=='payload' and isinstance(n.slice,ast.Constant) and isinstance(n.slice.value,str):keys.add(n.slice.value)
    keys.update(['edges','faces'])
    return sorted(keys)
if __name__=='__main__':
    mode=sys.argv[1];root=Path(sys.argv[2]);root.mkdir(parents=True,exist_ok=True)
    if mode=='prepare':
        source=build(root);keys=schema_keys();actual=document(source)['scene'];assert set(keys)<=set(actual),(set(keys)-set(actual))
        expected={}
        for kind in CASES:
            s=Scene();load_into(s,source);edit(s,kind);path=root/(kind+'-expected.igz');save_scene(s,path);expected[kind]=observation(path,root/(kind+'-source-reopen.igz'))
        (root/'expected.json').write_text(json.dumps({'source_pin':PIN,'source_keys':keys,'cases':expected},indent=2)+'\n')
        print('Captured all source fields:',len(keys),'cases:',len(CASES))
    elif mode=='controls':
        baseline=observation(root/'native/no-op.igz',root/'controls-baseline.igz')
        def changes(scene,kind):
            if kind=='custom-scales':scene['custom_scales']=[5.]
            elif kind=='saved-view-uid':scene['saved_views'][0]['hidden_objects']=[]
            elif kind=='group-extension':scene['groups'][0]['ext']={}
            elif kind=='photo-anchor':scene['photo_mesh']['anchor']['east']+=100.
        controls={}
        with zipfile.ZipFile(root/'native/no-op.igz') as z:members={n:z.read(n) for n in z.namelist()}
        for kind in ['custom-scales','saved-view-uid','group-extension','photo-anchor']:
            doc=json.loads(members['document.json']);changes(doc['scene'],kind)
            path=root/(kind+'-corrupt.igz')
            with zipfile.ZipFile(path,'w',zipfile.ZIP_DEFLATED) as z:
                for name,data in members.items():z.writestr(name,json.dumps(doc).encode() if name=='document.json' else data)
            changed=observation(path,root/(kind+'-corrupt-reopened.igz'))
            assert changed!=baseline,kind+' corruption was hidden by source observer'
            controls[kind]={'rejected':True,'observed':changed}
        (root/'controls.json').write_text(json.dumps(controls,indent=2)+'\n')
        print('Four corrupted IGZ archives rejected by actual source reopen')
    else:
        actual={kind:observation(root/(kind+'.igz'),root/(kind+'-python-reopen.igz'))for kind in CASES}
        (root/'observed.json').write_text(json.dumps(actual,indent=2)+'\n')
        expected=json.loads(Path(sys.argv[3]).read_text())['cases']
        def diff(a,b,path=''):
            if type(a)!=type(b):return [(path,'type',type(a).__name__,type(b).__name__)]
            if isinstance(a,dict):
                if set(a)!=set(b):return [(path,'keys',sorted(set(a)-set(b)),sorted(set(b)-set(a)))]
                return [d for k in a for d in diff(a[k],b[k],path+'/'+k)]
            if isinstance(a,list):
                if len(a)!=len(b):return [(path,'length',len(a),len(b))]
                return [d for i,(x,y)in enumerate(zip(a,b))for d in diff(x,y,path+'/'+str(i))]
            return [] if a==b else [(path,a,b)]
        differences={k:diff(expected[k],actual[k])for k in CASES};(root/'differences.json').write_text(json.dumps(differences,indent=2)+'\n');print(json.dumps(differences,indent=2));assert not any(differences.values())
