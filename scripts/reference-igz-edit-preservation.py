#!/usr/bin/env python3
"""Pinned operative IGZ edits and actual reader inventory; no native codec reuse."""
import sys,json,hashlib,copy,itertools
import core.group as group_module
from pathlib import Path
from PySide6.QtGui import QVector3D as V,QImage,QColor
from core.scene import Scene
from core.image_plane import ImagePlane
from formats.igz import load_into,save_scene,_mesh_json
ROOT=Path(__file__).resolve().parents[1]
def normalized(value):
    if isinstance(value,dict):
        result={k:normalized(v) for k,v in value.items()}
        for k in ('path','base'):
            if isinstance(result.get(k),str):
                p=Path(result[k])
                if p.is_file():result[k]={'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}
        return result
    if isinstance(value,(list,tuple)):return [normalized(v) for v in value]
    return value

def inventory(scene, authored=None):
    authored=authored or set()
    meshes={}
    def group(g):
        ident=id(g.mesh);shared=meshes.setdefault(ident,len(meshes))
        return {'name':g.name,'uid':g.uid if g.uid in authored else '<generated>','component':bool(getattr(g,'component',False)),
            'xform':list(g.xform.data()) if g.xform is not None else None,
            'mesh':_mesh_json(g.mesh),'share':shared,'children':[group(c) for c in getattr(g,'children',[])],
            'layer':getattr(g,'layer',None),'hidden':bool(getattr(g,'hidden',False))}
    return normalized({'mesh':_mesh_json(scene.mesh),'groups':[group(g) for g in scene.groups],
        'materials':[m.to_dict() for m in scene.materials.values()],
        'images':[p.to_dict() for p in scene.image_planes],
        'georef':scene.georef.to_dict() if scene.georef else None,
        'layers':[l.to_dict() for l in scene.layers]})

def edits(s,assets):
    e=s.mesh.add_edge(V(1000,0,0),V(1001,1,0));e.layer=12;e.curve=-3
    f=s.mesh.add_face([V(1100,0,0),V(1103,0,0),V(1103,2,0),V(1100,2,0)])
    f.attrs.update({'color':(.125,.5,.75),'opacity':.625,'layer':7,
        'texture':{'path':str(assets/'paint.png'),'base':str(assets/'base.png'),'sw':2,'sh':3,'rot':17},
        'back':{'color':(.75,.25,.125),'texture':{'path':str(assets/'back.png'),'sw':1,'sh':1}}})
    if s.groups:s.groups[0].name='Edited '+s.groups[0].name
    s.image_planes.append(ImagePlane(str(assets/'plane.png'),V(1200,0,0),V(4,0,0),V(0,2,0),name='Fresh reference',opacity=.375))

def prepare(out):
    out.mkdir(parents=True,exist_ok=True);assets=out/'assets';assets.mkdir(exist_ok=True)
    for i,name in enumerate(['paint','base','back','plane']):
        im=QImage(3,2,QImage.Format_RGBA8888);im.fill(QColor(40+i*30,100,170,255));im.save(str(assets/(name+'.png')))
    cases=[]
    for p in sorted((Path('/work/ingetrazo/examples')).glob('*.igz')):cases.append((p.stem,p))
    for name in ['nested-reflected','textures-alpha','pbr-colors']:
        cases.append((name,ROOT/'docs/audits/gltf-rich/original'/(name+'.igz')))
    fresh=out/'fresh.igz';save_scene(Scene(),fresh);cases.append(('fresh',fresh))
    numeric=Scene();f=numeric.mesh.add_face([V(0,0,0),V(2,0,0),V(0,2,0)]);f.attrs['layer']=9
    numeric.mesh.edges[0].layer=12
    path=out/'numeric-layers.igz';save_scene(numeric,path);cases.append(('numeric-layers',path))
    expected={};manifest=[]
    for name,path in cases:
        group_module._counter=itertools.count(1)
        s=Scene();load_into(s,path)
        # Existing stored UIDs must remain exact. Legacy missing UIDs are random
        # runtime identities; their save/reopen continuity is checked separately.
        import zipfile
        if zipfile.is_zipfile(path):raw=json.loads(zipfile.ZipFile(path).read('document.json'))
        else:raw=json.loads(path.read_text())
        def ids(node):
            found=set()
            if isinstance(node,dict):
                if isinstance(node.get('uid'),str):found.add(node['uid'])
                for v in node.values():found.update(ids(v))
            elif isinstance(node,list):
                for v in node:found.update(ids(v))
            return found
        authored=ids(raw)
        before=inventory(s,authored);edits(s,assets)
        dest=out/(name+'-expected.igz');save_scene(s,dest)
        reopened=Scene();load_into(reopened,dest)
        expected[name]={'before':before,'after':inventory(reopened,authored)}
        manifest.append({'name':name,'input':str(path),'assets':str(assets),'authored_uids':sorted(authored)})
    (out/'manifest.json').write_text(json.dumps(manifest,indent=2))
    (out/'expected.json').write_text(json.dumps(expected,indent=2))

def inspect(manifest, directory, output):
    result={}
    for case in json.loads(Path(manifest).read_text()):
        group_module._counter=itertools.count(1)
        s=Scene();load_into(s,Path(directory)/(case['name']+'.igz'))
        result[case['name']]=inventory(s,set(case.get('authored_uids',[])))
    Path(output).write_text(json.dumps(result,indent=2))
if __name__=='__main__':
    if sys.argv[1]=='prepare':prepare(Path(sys.argv[2]))
    else:inspect(*sys.argv[2:])
