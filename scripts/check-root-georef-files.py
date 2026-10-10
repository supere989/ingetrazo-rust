import sys,json,dataclasses,zipfile,subprocess
from pathlib import Path
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from georef.geoimport import load_features
from tests.test_geoimport import KML_LINE,KML_POLY
root=Path(sys.argv[2]);root.mkdir(parents=True,exist_ok=True)
files=[]
for name,content in [('line.KML',KML_LINE.encode()),('sniff.dat',b'  '+KML_POLY.encode()),('line.geojson',b'{"type":"LineString","coordinates":[[20,10],[21,11]]}'),('empty.json',b'{bad')]:
 p=root/name;p.write_bytes(content);files.append(p)
for name,entries in [('prefer-doc.kmz',[('first.kml',KML_LINE),('doc.kml',KML_POLY)]),('first-uppercase.KMZ',[('other.txt','ignore'),('FIRST.KML',KML_LINE),('second.kml',KML_POLY)]),('empty.kmz',[('readme.txt','empty')])]:
 p=root/name
 with zipfile.ZipFile(p,'w') as z:
  for member,content in entries:z.writestr(member,content)
 files.append(p)
expected=json.loads(json.dumps([[dataclasses.asdict(f) for f in load_features(p)]for p in files]))
actual=json.loads(subprocess.run([sys.argv[1],*map(str,files)],capture_output=True,text=True,check=True).stdout)
assert actual==expected
bad=json.loads(json.dumps(actual));bad[4]=bad[0];assert bad!=expected,'wrong KMZ first-entry choice accepted'
bad=json.loads(json.dumps(actual));bad[0][0]['points'][0].reverse();assert bad!=expected,'wrong latlon order accepted'
for name,data in [('original',expected),('native',actual)]: (root/(name+'.json')).write_text(json.dumps(data,indent=2)+'\n')
print(json.dumps({'exact_file_cases':len(files),'wrong_kmz_member_negative':True,'latlon_negative':True,'scope':'actual source file/KMZ loading; no GUI/projection/history claim'}))
