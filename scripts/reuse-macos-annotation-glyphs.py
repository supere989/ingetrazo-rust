"""Reuse pinned per-runner historical Qt glyph evidence; no new Qt execution."""
import hashlib,io,json,pathlib,platform,sys,zipfile
root=pathlib.Path(__file__).resolve().parents[1]
def records(arch):
 directory=root/'docs/audits/metal-face-sweep/apple-consolidated-guard'
 receipt=json.loads((directory/(arch+'-receipt.json')).read_text());raw=(directory/(arch+'.zip')).read_bytes()
 if len(raw)>6*1024*1024 or hashlib.sha256(raw).hexdigest()!=receipt['artifact_sha256']:raise ValueError('artifact integrity')
 outer=zipfile.ZipFile(io.BytesIO(raw));inner=outer.read('native-captures.zip')
 if len(inner)>6*1024*1024:raise ValueError('capture archive extent')
 archive=zipfile.ZipFile(io.BytesIO(inner));report=json.loads(archive.read('qt-glyphs/qt-glyphs.json'))
 if report['source_pin']!='6be29fe437a7cd992d4e079f71720821b25216b9' or report['qt']!='6.12.0' or len(report['cases'])!=4:raise ValueError('reference identity')
 files={'qt-glyphs.json':archive.read('qt-glyphs/qt-glyphs.json')}
 for case in report['cases']:
  name=case['image']
  if pathlib.PurePosixPath(name).name!=name:raise ValueError('image path')
  data=archive.read('qt-glyphs/'+name)
  if len(data)>1024*1024 or hashlib.sha256(data).hexdigest()!=case['image_sha256']:raise ValueError('image integrity')
  for run in case['glyph_runs']:
   for key in ['head_sha256','name_sha256']:
    if len(run[key])!=64 or any(c not in '0123456789abcdef' for c in run[key]):raise ValueError('font hash')
  files[name]=data
 if len(files)!=5:raise ValueError('record inventory')
 return receipt,report,files
if '--validate-only' in sys.argv:
 for arch in ['arm','intel']:
  receipt,report,files=records(arch);print(json.dumps({'arch':arch,'artifact_sha256':receipt['artifact_sha256'],'records':len(files),'platform':report['platform'],'scope':'retained byte/hash validation only'}))
else:
 if sys.platform!='darwin':raise SystemExit('Actual macOS required')
 arch='arm' if platform.machine()=='arm64' else 'intel' if platform.machine()=='x86_64' else None
 if arch is None:raise SystemExit('Unsupported runner architecture')
 receipt,report,files=records(arch)
 if report['machine']!=platform.machine() or not report['platform'].startswith('macOS-'+platform.mac_ver()[0]+'-'):raise SystemExit('Retained Qt oracle OS/architecture changed; reuse refused')
 out=pathlib.Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
 for name,data in files.items():(out/name).write_bytes(data)
 print(json.dumps({'reused_run':receipt['run'],'reused_artifact':receipt['artifact'],'artifact_sha256':receipt['artifact_sha256'],'records':len(files),'font_table_hashes':sorted(set((run['head_sha256'],run['name_sha256']) for case in report['cases']for run in case['glyph_runs'])),'scope':'historical actual Qt glyph reference, matching OS/architecture; no current Qt execution or current font certification'}))
