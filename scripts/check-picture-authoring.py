#!/usr/bin/env python3
"""Literal frozen picture authoring, original-reader and opaque-owner comparison."""
import argparse, copy, json, math, zipfile
from pathlib import Path
PIN = '6be29fe437a7cd992d4e079f71720821b25216b9'
NAMES = ['authoring', 'missing-fallback', 'aspect-selection']
COUNTS = [27, 7, 10]
CATEGORIES = ['frames','texts','images','scalebars','nortes','leyendas','shapes','cotas','cotas_ang','cotas_rad','etiquetas','perfiles','niveles','llamadas','cajetin']
FLAGS = ['actual_egui_add','actual_egui_border','actual_egui_replace','cancel_no_second_chooser','cancel_unchanged','replace_geometry_preserved','warm_scene_and_decode_composite_unchanged','warm_io_and_upload_unchanged','invalid_edits_preserve_history']

def equal(a, b, path='', known=False):
    assert type(a) is type(b), (path, 'type', type(a).__name__, type(b).__name__)
    if isinstance(a, dict):
        assert a.keys() <= b.keys() if known else a.keys() == b.keys(), (path, 'keys')
        for key in a: equal(a[key], b[key], path+'/'+key, known)
    elif isinstance(a, list):
        assert len(a) == len(b), (path, 'length')
        for i, (x,y) in enumerate(zip(a,b)): equal(x,y,path+'/'+str(i),known)
    else:
        if isinstance(a,float): assert math.isfinite(a) and math.isfinite(b),path
        assert a == b, (path, a, b)

def inventory(source, native):
    assert source['source_pin'] == PIN
    assert [r['name'] for r in source['cases']] == NAMES
    assert [len(r['actions']) for r in source['cases']] == COUNTS
    assert [r['name'] for r in native['cases']] == NAMES
    assert [r['saved'] for r in native['cases']] == [n+'-native.igz' for n in NAMES]
    expected=[(r['name'],a['phase']) for r in source['cases'] for a in r['actions']]
    assert len(expected)==44 and len(set(expected))==44
    for rows in [source['states'],native['states']]:
        assert [(r['case'],r['phase']) for r in rows] == expected
    assert type(native['fresh_source_captures']) is int and native['fresh_source_captures']==0

def differences(source, native, source_reader, native_reader, z, documents):
    errors=[]
    def check(label, fn):
        try: fn()
        except (AssertionError,KeyError,IndexError,TypeError,ValueError) as error: errors.append({'boundary':label,'error':str(error)})
    check('inventory',lambda:inventory(source,native))
    for i,(a,b) in enumerate(zip(source.get('states',[]),native.get('states',[]))):
        def phase(a=a,b=b):
            for key in ['case','phase','selection','undo','redo']:equal(a[key],b[key],key)
            equal(a['composition'],b['composition'],'composition',known=True)
        check('state/'+str(i)+'/'+a['phase'],phase)
    def reader():
        assert [r['case'] for r in source_reader]==NAMES
        assert [r['case'] for r in native_reader]==NAMES
        equal(source_reader,native_reader,'original-reader')
    check('original-reader',reader)
    def stacking():
        assert z['source_pin']==PIN
        assert [r['category'] for r in z['rows']]==CATEGORIES
        equal(z['rows'],native['z_controls'],'stacking')
    check('stacking',stacking)
    def controls():
        c=native['controls']
        for key in FLAGS: assert c[key] is True,key
        a,b=c['warm_counters_before'],c['warm_counters_after']
        assert type(a)is list and len(a)==5 and all(type(v)is int and v>=0 for v in a)
        equal(a,b,'warm-counters')
    check('actual-ui-and-warm-preview',controls)
    def opaque():
        assert [r['case'] for r in source['opaque_guards']]==['authoring']
        for guard in source['opaque_guards']:
            for path,value in guard['paths'].items():
                got=documents[guard['case']]
                for component in path.split('.'):
                    got=got[int(component)] if type(got)is list else got[component]
                equal(value,got,path)
    check('opaque-owner-preservation',opaque)
    return errors

def documents(native, root):
    result={}
    for row in native['cases']:
        path=root/row['saved'];path.resolve().relative_to(root.resolve())
        if zipfile.is_zipfile(path):
            with zipfile.ZipFile(path)as z:
                data=json.loads(z.read('scene.json'))
        else:data=json.loads(path.read_text())
        result[row['name']]=data if 'scene' in data else {'scene':data}
    return result

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('source',type=Path);p.add_argument('native',type=Path);p.add_argument('output',type=Path);args=p.parse_args()
    source=json.loads((args.source/'original-picture-authoring.json').read_text())
    native=json.loads((args.native/'native.json').read_text())
    original=json.loads((args.source/'original-reopens.json').read_text())
    reopened=json.loads((args.native/'native-reopens.json').read_text())
    z=json.loads((args.source/'original-z-controls.json').read_text())
    docs=documents(native,args.native)
    report={'result':'incomplete','states':44,'reader_pairs':3,'stacking_categories':15,'negative_controls':[],'scope':'Literal complete known composition fields, selection/history, actual egui controls with controlled picker replies, opaque native preservation and warm-preview counters. Source publication clock and whole-frame GPU pixels are separate.'}
    report['errors']=differences(source,native,original,reopened,z,docs)
    if not report['errors']:
        def state_mutation(b,phase,key,value):
            next(r for r in b['states']if r['case']=='authoring'and r['phase']==phase)[key]=value
        trials=[
            ('missing-case',lambda a,b,r,d:b['cases'].pop()),
            ('jointly-empty',lambda a,b,r,d:(a.update(cases=[],states=[]),b.update(cases=[],states=[]))),
            ('missing-phase',lambda a,b,r,d:b['states'].pop()),
            ('duplicate-phase',lambda a,b,r,d:b['states'].__setitem__(-1,b['states'][0])),
            ('boolean-selection',lambda a,b,r,d:state_mutation(b,'select-new','selection',True)),
            ('wrong-history',lambda a,b,r,d:state_mutation(b,'property-noop','undo',4)),
            ('lost-path',lambda a,b,r,d:r[0]['composition']['images'][1].update(path='lost.png')),
            ('lost-composition-field',lambda a,b,r,d:b['states'][0]['composition']['images'][0].pop('hidden')),
            ('boolean-opacity',lambda a,b,r,d:b['states'][0]['composition']['images'][0].update(opacity=True)),
            ('one-ulp-height',lambda a,b,r,d:b['states'][3]['composition']['images'][1].update(h_mm=math.nextafter(b['states'][3]['composition']['images'][1]['h_mm'],math.inf))),
            ('missing-category',lambda a,b,r,d:b['z_controls'].pop()),
            ('wrong-stacking',lambda a,b,r,d:b['z_controls'][8].update(new_image_z=-1.)),
            ('false-ui-control',lambda a,b,r,d:b['controls'].update(actual_egui_add=False)),
            ('numeric-ui-control',lambda a,b,r,d:b['controls'].update(cancel_unchanged=1)),
            ('warm-read-repeated',lambda a,b,r,d:b['controls']['warm_counters_after'].__setitem__(1,b['controls']['warm_counters_after'][1]+1)),
            ('opaque-loss',lambda a,b,r,d:d['authoring']['scene']['compositions'][0]['images'][0].pop('vendor_picture')),
        ]
        for name, mutate in trials:
            a,b,r,d=copy.deepcopy((source,native,reopened,docs));mutate(a,b,r,d)
            assert differences(a,b,original,r,z,d),'Corruption accepted: '+name
            report['negative_controls'].append(name)
    report['result']='fail'if report['errors']else'pass'
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report));return 0 if report['result']=='pass'else 1
if __name__=='__main__':raise SystemExit(main())
