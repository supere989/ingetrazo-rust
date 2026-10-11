#!/usr/bin/env python3
"""Capture only missing original DAE material contracts, or replay frozen inputs."""
import argparse, copy, hashlib, importlib.util, json, os, subprocess, sys, time, zipfile
import xml.etree.ElementTree as ET
from pathlib import Path

NS = '{http://www.collada.org/2005/11/COLLADASchema}'
ET.register_namespace('', NS[1:-1])
CASES = ['valid-image', 'missing-image', 'missing-image-no-uv', 'missing-unicode-image',
         'empty-image', 'absent-image-uri', 'missing-material', 'missing-effect',
         'missing-sampler', 'missing-surface', 'missing-image-id', 'literal-before-texture',
         'shader-priority', 'literal-malformed-percent', 'literal-plus-percent',
         'encoded-space', 'invalid-image', 'invalid-image-no-uv', 'directory-image',
         'shared-aliases', 'empty-duplicate-binding']

def sha(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def write(path, value): path.write_text(json.dumps(value, indent=2, ensure_ascii=False)+'\n')
def helper():
    spec=importlib.util.spec_from_file_location('dae_reference_helpers', 'scripts/check-python-dae-import.py')
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return module

def canonical(h, scene):
    result=h.signature(scene)
    result['textures']=sorted(result['textures'],key=lambda x:json.dumps(x,sort_keys=True))
    result['rings']=sorted(h.unoriented(r) for r in result['rings'])
    result['holes']=sorted(tuple(sorted(h.unoriented(r) for r in hs)) for hs in result['holes'])
    return json.loads(json.dumps(result))

def make_fixture(folder, name, h):
    folder.mkdir(parents=True)
    image=h.QImage(2,2,h.QImage.Format_RGBA8888);image.fill(h.QColor(203,71,29))
    for filename in ['paint.png', 'paint%GG.png', 'paint%+A.png', 'paint blue.png']: image.save(str(folder/filename))
    (folder/'broken.png').write_bytes(b'original accepts undecodable authored image bytes\n')
    (folder/'image-directory').mkdir()
    tree=ET.parse('docs/audits/dae-import-corpus/textured-uv.dae');root=tree.getroot()
    find=lambda path:root.find(path.replace('/', '/'+NS) if False else path)
    image_el=root.find('.//'+NS+'image');uri=image_el.find(NS+'init_from')
    effect=root.find('.//'+NS+'effect');tech=effect.find('.//'+NS+'technique')
    diffuse=root.find('.//'+NS+'diffuse');binding=root.find('.//'+NS+'instance_material')
    if name in ['missing-image','missing-image-no-uv']:uri.text='missing.png'
    elif name=='missing-unicode-image':uri.text='纹理🌳.png'
    elif name=='empty-image':uri.text=''
    elif name=='absent-image-uri':image_el.remove(uri)
    elif name=='missing-material':binding.set('target','#absent')
    elif name=='missing-effect':root.find('.//'+NS+'instance_effect').set('url','#absent')
    elif name=='missing-sampler':diffuse.find(NS+'texture').set('texture','absent')
    elif name=='missing-surface':root.find('.//'+NS+'sampler2D/'+NS+'source').text='absent'
    elif name=='missing-image-id':root.find('.//'+NS+'surface/'+NS+'init_from').text='absent'
    elif name=='literal-before-texture':ET.SubElement(diffuse,NS+'color').text='0.2 0.4 0.6 1'
    elif name=='shader-priority':
        # Source examines lambert first even when phong is earlier in the XML.
        phong=ET.Element(NS+'phong');ET.SubElement(ET.SubElement(phong,NS+'diffuse'),NS+'color').text='0.9 0.1 0.1 1';tech.insert(0,phong)
        ET.SubElement(diffuse,NS+'color').text='0.2 0.4 0.6 1'
    elif name=='literal-malformed-percent':uri.text='paint%GG.png'
    elif name=='literal-plus-percent':uri.text='paint%+A.png'
    elif name=='encoded-space':uri.text='paint%20blue.png'
    elif name in ['invalid-image','invalid-image-no-uv']:uri.text='broken.png'
    elif name=='directory-image':uri.text='image-directory'
    elif name=='shared-aliases':
        other=copy.deepcopy(image_el);other.set('id','img1');other.find(NS+'init_from').text='./paint.png';root.find(NS+'library_images').append(other)
        other_effect=copy.deepcopy(effect);other_effect.set('id','eff1');other_effect.find('.//'+NS+'surface/'+NS+'init_from').text='img1';root.find(NS+'library_effects').append(other_effect)
        material=copy.deepcopy(root.find('.//'+NS+'material'));material.set('id','mat1');material.find(NS+'instance_effect').set('url','#eff1');root.find(NS+'library_materials').append(material)
        node=copy.deepcopy(root.find('.//'+NS+'visual_scene/'+NS+'node'));node.set('id','model2');ET.SubElement(node,NS+'translate').text='8 0 0';node.find('.//'+NS+'instance_material').set('target','#mat1');root.find('.//'+NS+'visual_scene').append(node)
    elif name=='empty-duplicate-binding':
        ET.SubElement(diffuse,NS+'color').text='0.2 0.4 0.6 1'
        other_effect=copy.deepcopy(effect);other_effect.set('id','empty-effect');other_effect.find('.//'+NS+'diffuse').remove(other_effect.find('.//'+NS+'color'));root.find(NS+'library_effects').append(other_effect)
        uri.text=''
        material=copy.deepcopy(root.find('.//'+NS+'material'));material.set('id','empty-material');material.find(NS+'instance_effect').set('url','#empty-effect');root.find(NS+'library_materials').append(material)
        parent=root.find('.//'+NS+'bind_material/'+NS+'technique_common');other=copy.deepcopy(binding);other.set('target','#empty-material');parent.append(other)
    if name.endswith('-no-uv'):
        for prim in root.iter(NS+'triangles'):
            inputs=prim.findall(NS+'input');stride=max(int(i.get('offset','0')) for i in inputs)+1
            uv=next(i for i in inputs if i.get('semantic')=='TEXCOORD');offset=int(uv.get('offset'));indices=prim.find(NS+'p').text.split();prim.remove(uv)
            for item in inputs:
                if item is not uv and int(item.get('offset','0'))>offset:item.set('offset',str(int(item.get('offset'))-1))
            prim.find(NS+'p').text=' '.join(v for i,v in enumerate(indices) if i%stride!=offset)
    path=folder/'input.dae';tree.write(path,encoding='unicode');return path

def capture(root,h):
    assert os.environ.get('PYTHONHASHSEED')=='0', 'explicit original hash seed required'
    assert not root.exists(), 'source capture must use a new directory'
    root.mkdir(parents=True);rows=[]
    for name in CASES:
        path=make_fixture(root/name,name,h);scene=h.Scene();h.dae.load_dae(scene,path)
        observed=canonical(h,scene);h.igz.save_scene(scene,root/name/'original.igz')
        rows.append({'case':name,'source':observed})
    bindings=[{'path':str(p.relative_to(root)), 'bytes':p.stat().st_size,'sha256':sha(p)} for p in sorted(root.rglob('*')) if p.is_file()]
    report={'source_commit':subprocess.check_output(['git','-C','ingetrazo','rev-parse','HEAD'],text=True).strip(),'python':sys.version,'hash_seed':0,'helper_sha256':sha('scripts/check-python-dae-import.py'),'protocol_sha256':sha(__file__),'fixtures':bindings,'cases':rows,'scope':'21 missing material/binding/path cases; original winding canonicalized in the established open-face boundary, full material recipes and image bytes retained. Duplicate effect SID precedence is outside this finite corpus.'}
    write(root/'source.json',report);print(json.dumps({'source_cases':len(rows)}))

def replay(root,output,binary,h):
    assert os.environ.get('PYTHONHASHSEED')=='0'
    source=json.loads((root/'source.json').read_text());assert [r['case'] for r in source['cases']]==CASES
    for row in source['fixtures']:assert sha(root/row['path'])==row['sha256'],row['path']
    assert not output.exists(), 'retain each native attempt separately';output.mkdir(parents=True)
    rows=[];started=time.perf_counter()
    for row in source['cases']:
        name=row['case'];out=output/(name+'.igz')
        run=subprocess.run([str(binary),str(root/name/'input.dae'),str(out)],capture_output=True,text=True)
        result={'case':name,'returncode':run.returncode,'stdout':run.stdout,'stderr':run.stderr}
        if run.returncode==0:
            scene=h.Scene();h.igz.load_into(scene,out);observed=canonical(h,scene)
            with zipfile.ZipFile(out) as z:assets=[n for n in z.namelist() if n.startswith('textures/') and not n.endswith('/')]
            expected_assets=1 if row['source']['textures'] else 0
            # Byte ownership may retain an image used only for representative color.
            if name=='invalid-image-no-uv':expected_assets=1
            result.update(native=observed,match=observed==row['source'],owned_assets=len(assets),asset_match=len(assets)==expected_assets)
        else:result.update(match=False,asset_match=False)
        rows.append(result)
    controls=[]
    positive=next(r for r in rows if r['case']=='valid-image')
    if positive['match']:
        baseline=next(r['source'] for r in source['cases'] if r['case']=='valid-image')
        for field,value in [('textures',[]),('surface_recipes',[]),('bounds',[[1,5],[0,4],[0,0]])]:
            changed=copy.deepcopy(positive['native']);changed[field]=value
            controls.append({'case':'corrupt-'+field,'positive_passed':True,'changed':changed!=positive['native'],'rejected':changed!=baseline})
        recipe=copy.deepcopy(positive['native']);data=json.loads(recipe['surface_recipes'][0]);data['attrs']['texture']['uvw'][3]+=1;recipe['surface_recipes'][0]=json.dumps(data,sort_keys=True)
        controls.append({'case':'corrupt-uv-origin','positive_passed':True,'changed':recipe!=positive['native'],'rejected':recipe!=baseline})
    passed=all(r['match'] and r['asset_match'] for r in rows) and len(controls)==4 and all(r['changed'] and r['rejected'] for r in controls)
    report={'binary_sha256':sha(binary),'binary':str(binary),'source_sha256':sha(root/'source.json'),'seconds':time.perf_counter()-started,'cases':rows,'negative_controls':controls,'passed':passed}
    write(output/'report.json',report)
    print(json.dumps({'passed':passed,'cases':len(rows),'mismatches':[{k:r[k] for k in ['case','match','asset_match']} for r in rows if not r['match'] or not r['asset_match']],'negative_controls':len(controls),'seconds':report['seconds']}))
    assert passed, 'material resolution acceptance failed; attempt retained'

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('mode',choices=['capture','replay']);parser.add_argument('source',type=Path);parser.add_argument('--output',type=Path);parser.add_argument('--binary',type=Path);args=parser.parse_args()
    h=helper();app=h.QApplication([])
    if args.mode=='capture':capture(args.source,h)
    else:replay(args.source,args.output,args.binary,h)
