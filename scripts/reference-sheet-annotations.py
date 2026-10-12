#!/usr/bin/env python3
"""Missing-only six-family physical sheet annotation application exports."""
import argparse,hashlib,json,os,subprocess,sys
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('output',type=Path);a=p.parse_args();out=a.output.resolve();assert not out.exists();out.mkdir(parents=True)
for key,name in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:os.environ[key]=str(out/name)
os.environ['QT_QPA_PLATFORM']='offscreen';repo=Path(__file__).resolve().parents[1];original=repo/'ingetrazo';pin=subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip();assert pin=='6be29fe437a7cd992d4e079f71720821b25216b9';sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QImage,QFont,QFontInfo,QFontMetricsF,QPainter
import views.composer as composer_module
from core.composition import Composicion,field_values
from core.i18n import set_language
from views.viewport import Viewport
from views.composer import ComposerWindow,_line_count,_label_block_h,etiqueta_ink_w_mm,etiqueta_bg_rect_mm,text_baseline_rect_mm
from core.composition import expand_fields,PT_TO_MM
from formats import igz
operations=[];active_export=None
class ObservedPainter(QPainter):
 def note(self,kind,geometry):
  if active_export is None:return
  pen=self.pen();m=self.worldTransform();operations.append(dict(case=active_export[0],export=active_export[1],operation=kind,geometry=geometry,pen=dict(width=pen.widthF(),cap=pen.capStyle().value,join=pen.joinStyle().value,dash=list(pen.dashPattern()),color=pen.color().name(),alpha=pen.color().alpha()),matrix=[m.m11(),m.m12(),m.m13(),m.m21(),m.m22(),m.m23(),m.m31(),m.m32(),m.m33()]))
 def drawLine(self,*args):
  geometry=[[p.x(),p.y()]for p in args]if len(args)==2 else []
  self.note('drawLine',geometry);return super().drawLine(*args)
 def drawPath(self,path):
  self.note('drawPath',[[path.elementAt(i).type.value,path.elementAt(i).x,path.elementAt(i).y]for i in range(path.elementCount())]);return super().drawPath(path)
# Local observer delegates the original Qt operations; no original class/file/math is changed.
composer_module.QPainter=ObservedPainter
app=QApplication([]);app.setOrganizationName('IngeTrazoSheetAnnotationProof');app.setApplicationName('physical-six-families');set_language('en');os.chdir(out)
FAMILIES=['cotas','cotas_ang','cotas_rad','etiquetas','niveles','llamadas'];recipes=[]
def case(name,key=None,items=None,coverage=None,**extra):
 composition=dict(name=name,paper='A4',landscape=True,margin_mm=10.)
 if key:composition[key]=items
 composition.update(extra);recipes.append(dict(case=name,composition=composition,coverage=coverage or [],dpi=100,sheet_index=0))
def grid(values):
 return [dict(x_mm=40.+(i%3)*80.,y_mm=42.+(i//3)*58.,**v)for i,v in enumerate(values)]
for key in FAMILIES:case(key+'-default',key,[dict(x_mm=70.,y_mm=70.)],['default '+key])
case('cotas-positions','cotas',grid([
 dict(dx_mm=55.,dy_mm=0.,sep_mm=8.,text_pos='above',ends='arrow'),dict(dx_mm=50.,dy_mm=-25.,sep_mm=-6.,text_pos='below',ends='none'),dict(dx_mm=48.,dy_mm=20.,sep_mm=5.,text_pos='centered',ends='tick'),dict(dx_mm=0.,dy_mm=40.,text_pos='centered',text_align='horizontal',ends='arrow'),dict(dx_mm=35.,dy_mm=-20.,text_pos='aside',text_bg='#ead6b8',text_bg_opacity=.55,text_color='#30559b'),dict(dx_mm=-35.,dy_mm=25.,text_pos='aside_below',text='Custom <>',text_mm=3.4)]),['all five text positions','centered aligned/horizontal gap','arrow/tick/none','negative and oblique readable rotation','background opacity/text color'])
case('cotas-placement','cotas',grid([
 dict(dx_mm=35.,dy_mm=18.,axis='h',sep_mm=-8.,text_along='start'),dict(dx_mm=-20.,dy_mm=32.,axis='v',sep_mm=8.,text_along='end'),dict(dx_mm=35.,dy_mm=0.,text_along='end',text_dx_mm=3.,text_dy_mm=-3.,text='Dragged'),dict(dx_mm=3.,dy_mm=0.,text_pos='centered',text='Long words',text_align='horizontal'),dict(dx_mm=0.,dy_mm=0.,text_along='end',text='Zero',ends='none'),dict(dx_mm=2.,dy_mm=1.,sep_mm=0.,text_pos='centered',text_mm=5.)]),['forced H/V asymmetric extensions','start/end tail','dragged tail suppression','custom centered continuous line','automatic short gap fallback','zero normal/tail threshold'])
units=['m','cm','mm','in','ft','ft-in','in-frac','ft-in-frac'];case('cotas-units-scale','cotas',[dict(x_mm=35.,y_mm=25.+i*18.,dx_mm=70.,scale_n=25.,units=u,decimals=3,offset_mm=1.,text='Length <>')for i,u in enumerate(units)]+[dict(x_mm=170.,y_mm=55.,dx_mm=60.,scale_n=25000.,text_mm=4.,text='Road <>')],['all eight authored unit codes','custom value substitution','scale25 and kilometer threshold'])
case('angular-sweep','cotas_ang',grid([
 dict(ax_mm=25.,ay_mm=0.,bx_mm=0.,by_mm=-25.,ends='arrow'),dict(ax_mm=25.,ay_mm=0.,bx_mm=0.,by_mm=25.,ends='tick'),dict(ax_mm=25.,ay_mm=0.,bx_mm=-25.,by_mm=0.,ends='none'),dict(ax_mm=0.,ay_mm=0.,bx_mm=20.,by_mm=15.,radius_mm=.5,ends='none'),dict(ax_mm=5.,ay_mm=3.,bx_mm=-3.,by_mm=-5.,radius_mm=20.,text='Angle <>',decimals=3,text_bg='#d8e4f7',text_bg_opacity=.6,text_color='#923233')]),['positive/negative/pi short sweep','zero ray skip/radius clamp','short ray extension','arrow/tick/none','custom angle/bg/color'])
case('radial-fit','cotas_rad',grid([
 dict(radius_mm=32.,angle_deg=-35.,scale_n=10.,placement='auto'),dict(radius_mm=3.,angle_deg=25.,scale_n=100.,placement='auto'),dict(radius_mm=20.,angle_deg=145.,kind='diameter',placement='in'),dict(radius_mm=18.,angle_deg=90.,kind='diameter',placement='out',ends='tick'),dict(radius_mm=12.,angle_deg=-90.,placement='in',ends='none',centre_mark=False),dict(radius_mm=12.,angle_deg=-145.,placement='out',text='Radius <>',units='in-frac',decimals=2,text_bg='#e1dda4',text_bg_opacity=.65,text_color='#67388f')]),['auto fits/does not fit','forced in/out','radius/diameter right-half text','all quadrants/exact vertical readable angle','arrow/tick/none','center mark on/off','custom units/bg/color'])
case('labels-leaders','etiquetas',grid([
 dict(text='LEFT',ax_mm=-15.,ay_mm=8.),dict(text='RIGHT',ax_mm=50.,ay_mm=8.,arrow=False),dict(text='ABOVE',ax_mm=5.,ay_mm=-15.,dot=False),dict(text='BELOW',ax_mm=5.,ay_mm=22.,arrow=False,dot=False),dict(text='MULTI',ax_mm=-12.,ay_mm=10.,leaders=[dict(ax_mm=40.,ay_mm=-10.),dict(ax_mm=5.,ay_mm=25.)],stroke_mm=.6)]),['all leader attachment sides','arrow and landing dot independent','extra multileaders','thick round caps'])
case('labels-wrapped','etiquetas',[dict(x_mm=35.,y_mm=30.,w_mm=30.,text='Café long words must wrap\nTyped second line',size_pt=12.,bold=True,italic=True,underline=True,bg_color='#f0d8aa',bg_opacity=.6,ax_mm=-15.,ay_mm=35.),dict(x_mm=140.,y_mm=40.,w_mm=70.,text='Two words',size_pt=20.,bg_color='#dce8ff',bg_opacity=1.,ax_mm=50.,ay_mm=50.),dict(x_mm=40.,y_mm=135.,text='',bg_color='#dddddd',arrow=False,dot=False)],['word wrapping and typed newline','bold italic underline','ink-hugging slab versus wrap width','empty text/background branch'])
case('levels-symbols','niveles',grid([
 dict(),dict(symbol='circle',mirror=True,z_m=-.3,ax_mm=-14.,ay_mm=12.),dict(symbol='triangle',mirror=True,z_m=2.3456,decimals=3,text='LONG PREFIX',line_mm=3.,size_mm=4.),dict(symbol='circle',z_m=.004,decimals=2,ax_mm=.3,ay_mm=0.),dict(symbol='triangle',z_m=.006,decimals=2,datum_m=.001,text=''),dict(symbol='circle',anchor_uid='unresolved-preserved',a_world=[1.,2.,7.25],datum_m=2.,z_m=100.,text='Datum {z}',color='#4b5488')]),['triangle and circle quadrants','mirrored text/line','leader strict .3 threshold','signed values and precision-zero ±','missing/empty {z} text','long label grows line','stored anchored Z/datum without resolving a model frame'])
case('callouts-leaders','llamadas',grid([
 dict(w_mm=30.,h_mm=20.,bx_mm=38.,by_mm=-8.,sheet='L-05'),dict(shape='circle',w_mm=30.,h_mm=20.,bx_mm=38.,by_mm=-8.,number='2',sheet=''),dict(w_mm=30.,h_mm=20.,bx_mm=15.,by_mm=10.,number='3',sheet='IN'),dict(shape='circle',w_mm=30.,h_mm=20.,bx_mm=15.,by_mm=10.,number='4'),dict(w_mm=30.,h_mm=20.,bx_mm=31.,by_mm=10.,number='NEAR',size_mm=4.,sheet=''),dict(shape='circle',w_mm=40.,h_mm=15.,bx_mm=-12.,by_mm=8.,number='6',sheet='L-07',stroke_mm=.6,color='#755027')]),['dashed rounded rect/circle','outside leader','inside rect/ellipse suppression','near bubble suppression','bubble split versus unsplit','bubble size/color/thick dash'])
# Field ownership uses hidden metadata frames; no Model View pixels or anchor synchronization claimed.
case('field-context',etiquetas=[dict(x_mm=40.,y_mm=40.,text='{nombre} | {hoja}/{total}\n{escala} {escena} {archivo} {unknown}',w_mm=70.,bg_color='#eeeeee')],llamadas=[dict(x_mm=160.,y_mm=75.,number='{escala}',sheet='{escena}',frame_uid='bound',bx_mm=45.,by_mm=-12.)],frames=[dict(uid='main',hidden=True,view_key='scene:Facade',scale_n=50.,w_mm=100.,h_mm=80.),dict(uid='bound',hidden=True,view_key='scene:Detail',scale_n=.5,w_mm=40.,h_mm=30.)],coverage=['sheet name/index/total/file','main versus bound frame scale/scene','unknown fields remain literal'])
recipes[-1]['sheet_index']=1;recipes[-1]['preceding_sheet']=dict(name='Preceding',paper='A4',landscape=True)
all_defaults={k:[dict(x_mm=65.+(i%3)*75.,y_mm=55.+(i//3)*75.)]for i,k in enumerate(FAMILIES)};case('all-six-defaults',coverage=['joint six-family export'],**all_defaults)
for name,high,hidden in [('mixed-same-z',False,False),('mixed-high-z',True,False),('all-six-hidden',False,True)]:
 items={k:[dict(x_mm=100.,y_mm=80.,z=float(10-i if high else 0),hidden=hidden)]for i,k in enumerate(FAMILIES)}
 items['shapes']=[dict(kind='rect',x_mm=90.,y_mm=65.,w_mm=70.,h_mm=55.,fill=True,fill_color='#e4daba',z=0.,hidden=hidden)];case(name,coverage=['all-family stable category ordering'if not high else 'cross-family z reorder','hidden painter exclusion'if hidden else 'overlap'],**items)
rows=[];reopens=[];windows=[]
for r in recipes:
 name=r['case'];host=QMainWindow();host.viewport=Viewport();scene=host.viewport.scene;comp=Composicion.from_dict(r['composition']);scene.compositions=([Composicion.from_dict(r['preceding_sheet'])]if r.get('preceding_sheet')else[])+[comp];host._current_path=Path('fixture-project.igz');window=ComposerWindow(host);windows.append((window,host));window._auto_render=False;window._auto_timer.stop();window.show_sheet(r['sheet_index']);before=[c.to_dict()for c in scene.compositions];active_export=(name,'png');window.export_image(str(out/(name+'.png')),r['dpi']);active_export=(name,'jpeg');window.export_image(str(out/(name+'.jpg')),r['dpi']);active_export=(name,'pdf');window.export_pdf(str(out/(name+'.pdf')));active_export=None;assert [c.to_dict()for c in scene.compositions]==before
 igz.save_scene(scene,out/(name+'.igz'));restored=type(scene)();igz.load_into(restored,out/(name+'.igz'));reopens.append(dict(case=name,compositions=[c.to_dict()for c in restored.compositions]))
 image=QImage(str(out/(name+'.png'))).convertToFormat(QImage.Format_RGB888);assert not image.isNull();data=bytes(image.constBits());rgb=b''.join(data[y*image.bytesPerLine():y*image.bytesPerLine()+image.width()*3]for y in range(image.height()))
 rows.append(dict(case=name,dpi=r['dpi'],sheet_index=r['sheet_index'],scene=name+'.igz',document_path='fixture-project.igz',image=dict(file=name+'.png',size=[image.width(),image.height()],rgb_sha256=hashlib.sha256(rgb).hexdigest()),jpeg=name+'.jpg',pdf=name+'.pdf',composition=comp.to_dict(),field_values=field_values(),bound_field_values=field_values('bound')if name=='field-context'else None,label_metrics=[dict(index=i,expanded_text=expand_fields(et.text),line_count=_line_count(expand_fields(et.text),float(et.w_mm),et.size_pt*PT_TO_MM,et.bold),block_height=_label_block_h(et),ink_width=etiqueta_ink_w_mm(et),background_rect=[etiqueta_bg_rect_mm(et).x(),etiqueta_bg_rect_mm(et).y(),etiqueta_bg_rect_mm(et).width(),etiqueta_bg_rect_mm(et).height()])for i,et in enumerate(comp.etiquetas)],baseline_probes=[dict(text_mm=size,rect=[text_baseline_rect_mm(size,-.5).x(),text_baseline_rect_mm(size,-.5).y(),text_baseline_rect_mm(size,-.5).width(),text_baseline_rect_mm(size,-.5).height()])for size in [2.5,2.8,4.]],recipe=r,nonmutating=True));window.hide();host.hide();window.deleteLater();host.viewport.deleteLater();host.deleteLater();app.processEvents();(out/'source.partial.json').write_text(json.dumps(dict(images=rows),ensure_ascii=False,indent=2)+'\n')
font=QFont('Sans Serif');font.setPixelSize(100);info=QFontInfo(font);fm=QFontMetricsF(font);report=dict(source_pin=pin,images=rows,font=dict(requested='Sans Serif',resolved=info.family(),pixel_size=info.pixelSize(),ascent=fm.ascent(),descent=fm.descent(),line_spacing=fm.lineSpacing(),cap_height=fm.capHeight()),scope='Actual unchanged ComposerWindow PNG/JPEG/PDF exports and original IGZ readers. Source fixture assignments declared; field context has metadata-only hidden frames, stored unresolved anchors are not Model View binding proof. No UI authoring or physical OS input claim.')
(out/'operator-observations.json').write_text(json.dumps(dict(scope='Bounded observational drawLine/drawPath call identity, actual Qt pen/path/matrix; delegates original methods without source class/file/math changes. PNG/JPEG/PDF are actual ComposerWindow exports.',operations=operations),indent=2)+'\n')
(out/'source.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');(out/'original-reopens.json').write_text(json.dumps(reopens,ensure_ascii=False,indent=2)+'\n');print(json.dumps(dict(cases=len(rows),families=FAMILIES)))
