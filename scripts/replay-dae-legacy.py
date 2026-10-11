#!/usr/bin/env python3
"""Replay frozen DAE corpora with authenticated per-case resources; never recapture source."""
import argparse,hashlib,importlib.util,json,os,shutil,subprocess,sys,zipfile,xml.etree.ElementTree as ET
from pathlib import Path
sys.path.insert(0,str(Path(__file__).parent));os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def load(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m

def main():
 ap=argparse.ArgumentParser();ap.add_argument('binary',type=Path);ap.add_argument('output',type=Path);a=ap.parse_args();assert not a.output.exists();a.output.mkdir(parents=True)
 h=load('import_helpers','scripts/check-python-dae-import.py');ref=load('reference_helpers','scripts/check-python-dae-reference.py');app=h.QApplication([])
 cohorts=[('small','dae-import-corpus','python-dae-import-evidence.json',12),('reference','dae-reference-corpus','python-dae-reference-evidence.json',7),('uv','dae-uv-fit-corpus','dae-uv-fit-evidence.json',4)]
 rows=[];bindings=[]
 for cohort,corpus,evidence,count in cohorts:
  root=Path('docs/audits')/corpus;ep=Path('docs/audits')/evidence;source=json.loads(ep.read_text());assert len(source['cases'])==count;bindings.append({'path':str(ep),'sha256':sha(ep)})
  for case in source['cases']:
   name=case['case'];folder=a.output/cohort/name;folder.mkdir(parents=True);inp=root/(name+'.dae');shutil.copy2(inp,folder/inp.name);bindings.append({'path':str(inp),'sha256':sha(inp)})
   expected=case['original'];resources=[];igz=root/(name+'.igz');texture_hashes={t['hash'] for t in expected['textures']}
   encoded={}
   if zipfile.is_zipfile(igz):
    with zipfile.ZipFile(igz)as z:
     for member in z.namelist():
      if member.startswith('textures/') and not member.endswith('/'):
       data=z.read(member);encoded[hashlib.sha256(data).hexdigest()]=(data,member)
   refs=[n.text.strip() for n in ET.parse(inp).getroot().iter() if n.tag.endswith('init_from') and n.text and n.text.strip().endswith('.png')]
   for image in refs:
    path=folder/image;path.parent.mkdir(parents=True,exist_ok=True)
    if texture_hashes:
     assert len(texture_hashes)==1 and texture_hashes<=encoded.keys(),(name,'missing retained asset')
     digest=next(iter(texture_hashes));data,member=encoded[digest];path.write_bytes(data);method='retained corresponding IGZ member '+member;assert sha(path)==digest
    elif cohort=='small' and name.startswith('textured-missing-uv'):
     w,ht=map(int,name.rsplit('gradient',1)[1].split('x'))if'gradient'in name else(2,2);image_q=h.QImage(w,ht,h.QImage.Format_RGBA8888);image_q.fill(h.QColor(203,71,29))
     if name.endswith('pattern')or name.endswith('alpha'):
      for(x,y),rgba in zip([(0,0),(1,0),(0,1),(1,1)],[(255,0,0,255),(0,255,0,128 if name.endswith('alpha')else 255),(0,0,255,64 if name.endswith('alpha')else 255),(255,255,255,0 if name.endswith('alpha')else 255)]):image_q.setPixelColor(x,y,h.QColor(*rgba))
     if'gradient'in name:
      for x in range(w):
       for y in range(ht):image_q.setPixelColor(x,y,h.QColor((x*61+y*17+11)%256,(x*7+y*53+19)%256,(x*43+y*29+23)%256))
     assert image_q.save(str(path));method='literal frozen QImage recipe; historical source record is color-only and retains no image byte hash'
    else:
     shutil.copy2(root/image,path);method='stable retained corpus asset'
    resources.append({'path':image,'sha256':sha(path),'method':method,'expected_image_hash_present':bool(texture_hashes)})
   out=folder/'native.igz';run=subprocess.run([str(a.binary),str(folder/inp.name),str(out)],capture_output=True,text=True)
   row={'cohort':cohort,'case':name,'returncode':run.returncode,'stdout':run.stdout,'stderr':run.stderr,'input_sha256':sha(inp),'resources':resources}
   if run.returncode==0:
    scene=h.Scene();h.igz.load_into(scene,out);observed=ref.summary(scene)if cohort=='reference'else h.signature(scene)
    schema=h.editable(expected,observed);oriented=expected['rings']==observed['rings'] and expected['holes']==observed['holes']
    row.update(native=observed,world_match=all(expected[k]==observed[k]for k in['area','bounds','color_area','textures']),schema_match=schema and(name!='closed-cube'or oriented),strict_oriented_rings_match=oriented)
   else:row.update(world_match=False,schema_match=False)
   rows.append(row)
 report={'binary_sha256':sha(a.binary),'source_bindings':bindings,'cases':rows,'passed':all(r['world_match']and r['schema_match']for r in rows),'scope':'23 retained source cases, original IGZ reader reopen; open-ring nondeterministic boundary unchanged, closed-cube oriented rings strict. No original DAE recapture or new runtime build.'}
 (a.output/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'passed':report['passed'],'cases':len(rows),'failures':[r['case']for r in rows if not r['world_match']or not r['schema_match']]}));assert report['passed']
if __name__=='__main__':main()
