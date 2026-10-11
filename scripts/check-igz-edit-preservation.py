#!/usr/bin/env python3
"""One strict acceptance function, reused for altered observed results."""
import json,sys,copy,zipfile,hashlib,gzip
from pathlib import Path

def validate(expected,observed):
    assert set(expected)==set(observed),'case inventory differs'
    for name,e in expected.items():assert e['after']==observed[name],f'{name}: operative reopened document differs'

def archive_controls(manifest,directory):
    def document(path):
        if zipfile.is_zipfile(path):
            with zipfile.ZipFile(path) as z:return json.loads(z.read('document.json'))['scene']
        return json.loads(Path(path).read_text())['scene']
    report={}
    for c in json.loads(Path(manifest).read_text()):
        path=Path(directory)/(c['name']+'.igz');assert zipfile.is_zipfile(path),'New face/back/base/plane images require a ZIP'
        old=document(c['input']);new=document(path)
        preserved=[]
        for key in old:
            if key not in {'edges','faces','groups','protos','materials','image_planes'}:
                assert old[key]==new[key],f"Unedited document field changed: {c['name']}/{key}"
                preserved.append(key)
        with zipfile.ZipFile(path) as z:
            blobs={n:hashlib.sha256(z.read(n)).hexdigest() for n in z.namelist() if n!='document.json'}
            refs=[]
            def walk(node):
                if isinstance(node,dict):
                    for key,v in node.items():
                        if key=='texture' and isinstance(v,dict):
                            for field in ('embed','base_embed'):
                                if isinstance(v.get(field),str):assert v[field] in blobs,f'Dangling {field}';refs.append(v[field])
                            for field in ('path','base'):
                                assert not str(v.get(field,'')).startswith(c['assets']),f'New asset still machine-local: {field}'
                        elif key not in {'edges','vertices','holes','a','b'}:walk(v)
                elif isinstance(node,list):
                    for v in node:walk(v)
            walk(new)
        report[c['name']]={'members':len(blobs),'embedded_references':len(refs),'preserved_document_fields':sorted(preserved),'asset_sha256':sorted(set(blobs.values()))}
    return report

def main():
    expected=json.load(gzip.open(sys.argv[1],'rt')) if sys.argv[1].endswith('.gz') else json.loads(Path(sys.argv[1]).read_text())
    if sys.argv[1].endswith('.gz'):expected={k:{'after':v} for k,v in expected.items()}
    actual=json.loads(Path(sys.argv[2]).read_text())
    validate(expected,actual)
    def reject(value):
        try:validate(expected,value)
        except AssertionError:return
        raise AssertionError('mutated result accepted')
    bad=copy.deepcopy(actual);bad.pop(next(iter(bad)));reject(bad)
    for field in ['texture','back','layer']:
        bad=copy.deepcopy(actual);bad['fresh']['mesh']['faces'][-1].pop(field);reject(bad)
    bad=copy.deepcopy(actual);bad['fresh']['images'].clear();reject(bad)
    bad=copy.deepcopy(actual);bad['pbr-colors']['georef']=None;reject(bad)
    bad=copy.deepcopy(actual);bad['nested-reflected']['groups'][0]['name']='wrong';reject(bad)
    bad=copy.deepcopy(actual);bad['fresh']['mesh']['edges'][-1]['layer']=0;reject(bad)
    report={'cases':len(actual),'negative_controls':8,'strict_reopen_equal':True}
    if len(sys.argv)>4:report['archive']=archive_controls(sys.argv[3],sys.argv[4])
    print(json.dumps(report))
if __name__=='__main__':main()
