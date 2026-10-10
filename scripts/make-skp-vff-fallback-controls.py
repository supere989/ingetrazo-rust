#!/usr/bin/env python3
"""Source-supported missing image fallback, with actual joined materials."""
import runpy,sys,json
from pathlib import Path
ns=runpy.run_path(str(Path(__file__).with_name('make-skp-vff-positioned-controls.py')),run_name='__main__')
rows=ns['rows'];tl=ns['tl'];file=ns['file'];geom=ns['geom'];mat=ns['mat'];out=Path(sys.argv[2])
rows.append(file('texture-missing-painted',mat+tl('F601',geom(material=99)),{'materials/Tile/material.xml':ns['ns']['xml']()}))
rows.append(file('texture-missing-inherited',mat+ns['ns']['model']([ns['ns']['leaf'](1)],ns['ns']['ins'](5000,20000,paint=99)),{'materials/Tile/material.xml':ns['ns']['xml']()}))
rows.append(file('texture-empty-painted',mat+tl('F601',geom(material=99)),{'materials/Tile/material.xml':ns['ns']['xml'](),'materials/Tile/tile.png':b''}))
manifest=json.loads((out/'manifest.json').read_text());manifest['stage']='fallback-v1';manifest['cases']=rows;(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
