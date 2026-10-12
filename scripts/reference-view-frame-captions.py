#!/usr/bin/env python3
"""Missing physical ViewFrame captions/empty placeholder: actual pinned App exports."""
import os,sys,json,hashlib,subprocess
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
root=Path(__file__).resolve().parents[1];sys.path.insert(0,str(root/'ingetrazo'))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V,QImage
from core.composition import Composicion,MarcoVista
from views.viewport import Viewport
from views.composer import ComposerWindow
from formats import igz
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
pin='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C',str(root/'ingetrazo'),'rev-parse','HEAD'],text=True).strip()==pin
recipes=[
 ('title-layout-left',dict(title_number='3',title_sheet='A101',title_subtitle='Plan detail')),
 ('title-layout-center-above',dict(title_align='center',title_pos='above',title_number='4',title_subtitle='Upper caption')),
 ('title-layout-right',dict(title_align='right',title_number='5',title_scale=False)),
 ('title-simple-left',dict(title_style='simple',title_align='left',title_subtitle='Simple subtitle')),
 ('title-simple-right-above',dict(title_style='simple',title_align='right',title_pos='above',title_text='A deliberately long title which wraps within the view rectangle')),
 ('title-bar',dict(title_style='bar',title_number='6',title_sheet='A102',title_subtitle='Vertical caption')),
 ('title-fields',dict(title_text='{nombre} / {lamina}',title_subtitle='Scale {escala}; sheet {hoja} of {total}',title_number='7')),
 ('scale-under-left',dict(show_title=False,show_scale=True,scale_pos='under-left',scale_text='Scale {n}')),
 ('scale-under-right-enlargement',dict(show_title=False,show_scale=True,scale_n=.1,scale_pos='under-right')),
 ('scale-inside-bl',dict(show_title=False,show_scale=True,scale_pos='inside-bl')),
 ('scale-inside-br',dict(show_title=False,show_scale=True,scale_pos='inside-br',scale_text='Custom 1:{n}')),
 ('empty-view',dict(show_title=False,empty=True)),
 ('empty-title-scale',dict(title_style='simple',show_scale=True,scale_pos='inside-bl',empty=True)),
]
app=QApplication([]);rows=[]
corpus=root/'docs/audits/sheet-view-vector-pdf/corpus';ref=json.loads((corpus/'front_box.reference.json').read_text())
for name,props in recipes:
 host=QMainWindow();host.viewport=Viewport();vp=host.viewport
 opts=props.copy();empty=opts.pop('empty',False)
 if not empty:igz.load_into(vp.scene,corpus/'front_box.igz')
 for field,value in ref['camera'].items():setattr(vp.camera,field,V(*value)if field in ('target','up')else value)
 comp=Composicion(name=name,paper='A4',landscape=False)
 args=dict(x_mm=45.,y_mm=65.,w_mm=110.,h_mm=100.,view_key='__current__',style='vectorial',scale_n=100.,profiles=True,hidden_lines=False,cut_fill='none',annotations=False,section_marks=False,border=True,border_mm=.43,border_color='#246891',show_title=True,title_text='Physical view',title_mm=4.)
 args.update(opts);frame=MarcoVista(**args);comp.frames=[frame];vp.scene.compositions=[comp]
 window=ComposerWindow(host);window.refresh_all_frames();window.export_image(str(out/(name+'.png')),100);window.export_pdf(str(out/(name+'.pdf')));igz.save_scene(vp.scene,out/(name+'.igz'))
 cam={field:getattr(vp.camera,field)for field in ref['camera']};cam['target']=[vp.camera.target.x(),vp.camera.target.y(),vp.camera.target.z()];cam['up']=[vp.camera.up.x(),vp.camera.up.y(),vp.camera.up.z()]
 image=QImage(str(out/(name+'.png'))).convertToFormat(QImage.Format_RGB888);raw=bytes(image.constBits());rgb=b''.join(raw[y*image.bytesPerLine():y*image.bytesPerLine()+image.width()*3]for y in range(image.height()))
 rows.append(dict(case=name,dpi=100,scene=name+'.igz',camera=cam,image=dict(file=name+'.png',size=[image.width(),image.height()],rgb_sha256=hashlib.sha256(rgb).hexdigest()),pdf=name+'.pdf',recipe=props,hlr_segments=len(window.hlr_cache.get(id(frame),[]))))
 window.close();host.close()
(out/'source.json').write_text(json.dumps(dict(source_pin=pin,images=rows,scope='Actual pinned ComposerWindow refresh_all_frames/export_image/export_pdf. Existing front_box model reused; only missing labels/empty sessions captured.'),indent=2)+'\n')
print(json.dumps({'cases':len(rows),'names':[r['case']for r in rows]}))
