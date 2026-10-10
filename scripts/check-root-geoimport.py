import sys,json,subprocess,dataclasses
from pathlib import Path
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from georef.geoimport import parse_kml,parse_geojson
from tests.test_geoimport import KML_LINE,KML_POLY
cases=[{'format':'kml','text':t} for t in [KML_LINE,KML_POLY,'<broken']]
geometries=[
 {'type':'LineString','coordinates':[[20,10,99],[21,11]]},
 {'type':'MultiLineString','coordinates':[[[1,2],[3,4]],[[5,6],[7,8]]]},
 {'type':'Polygon','coordinates':[[[1,2],[3,4],[5,6],[1,2]],[[20,20],[30,30],[40,40]]]},
 {'type':'MultiPolygon','coordinates':[[[[1,2],[3,4],[5,6],[1,2]]],[[[7,8],[9,10],[11,12]]]]},
 {'type':'GeometryCollection','geometries':[{'type':'LineString','coordinates':[[20,10],[21,11]]},{'type':'Point','coordinates':[0,0]}]},
]
cases.extend({'format':'json','text':json.dumps({'type':'Feature','properties':{'name':'Canal Δ'},'geometry':g})}for g in geometries)
cases.extend([{'format':'json','text':'{broken'},{'format':'kml','text':'<kml><Placemark><name> A </name><MultiGeometry><LineString><coordinates>bad 4,3,8 6,5</coordinates></LineString><Polygon><coordinates>1,2 3,4 5,6 1.0000000005,2</coordinates></Polygon></MultiGeometry></Placemark></kml>'}])
expected=[[dataclasses.asdict(f) for f in (parse_kml if c['format']=='kml' else parse_geojson)(c['text'])]for c in cases]
expected=json.loads(json.dumps(expected))
p=subprocess.run([sys.argv[1]],input=json.dumps(cases),text=True,capture_output=True,check=True);actual=json.loads(p.stdout)
assert actual==expected,(actual,expected)
out=Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True)
for name,data in [('cases',cases),('original',expected),('native',actual)]: (out/(name+'.json')).write_text(json.dumps(data,indent=2)+'\n')
print(json.dumps({'exact_cases':len(cases),'scope':'original parser vs native parser; no import UI/projection/KMZ claim'}))
