import os,sys,json,importlib.util
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'ingetrazo')
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D
from core import extensions
from core import render_blender as rb
from formats.igz import save_scene,load_into
from core.scene import Scene
app=QApplication([]);extensions.plugin_dirs=lambda:[]
from views.main_window import MainWindow
from views.extension_api import ExtensionApp
spec=importlib.util.spec_from_file_location('_render_plugin',Path('ingetrazo/plugins/render_blender.py'));mod=importlib.util.module_from_spec(spec);spec.loader.exec_module(mod)
win=MainWindow();bridge=ExtensionApp(win,'render_blender');panel=mod.RenderPanel(bridge)
class Raw:
 def __init__(self,v):self.v=v
 def document_data(self,default):return self.v
cases=json.loads(Path('docs/audits/blender-document-follow/cases.json').read_text());clean=[mod._state(Raw(v))for v in cases]
rows=[]
def snap(name):rows.append({'name':name,'document':mod._state(bridge)})
snap('initial');panel.add_light('point',QVector3D(1,2,3),True);snap('point');panel.add_light('spot',QVector3D(4,5,6));snap('spot')
st=mod._state(bridge);st['ambience']='night';st['sun_scale']=1.5;st['lights'][1]['dir']=[2.,-1.,-3.];mod._store(bridge,st);snap('edited')
win.viewport.history.undo();snap('undo');win.viewport.history.redo();snap('redo');panel.delete_light_at(0);snap('delete');win.viewport.history.undo();snap('undo-delete')
output=Path(sys.argv[1]);save_scene(win.viewport.scene,output);again=Scene();load_into(again,output);reopen=mod._state(Raw(again.plugin_data['render_blender']))
from core.camera import OrbitCamera
cameras=[]
for target in [[0.1,0.2,0.3],[123.456,-12.345,0.125]]:
 cam=OrbitCamera();cam.target=QVector3D(*target);cameras.append({'camera':rb.camera_dict(cam),'key':rb.camera_key(cam)})
native_scene=Scene();load_into(native_scene,Path(sys.argv[2]));assert mod._state(Raw(native_scene.plugin_data['render_blender']))==reopen
print(json.dumps({'clean':clean,'states':rows,'reopen':reopen,'cameras':cameras}));panel.deleteLater();win._saved_version=win.viewport.scene.version;win.close()
