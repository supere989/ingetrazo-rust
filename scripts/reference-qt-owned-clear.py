#!/usr/bin/env python3
"""Bounded actual QPainter/Cocoa-owned-window diagnostic, never presentation proof."""
import argparse, ctypes, ctypes.util, hashlib, importlib.util, json, platform, struct, subprocess, sys, time
from pathlib import Path

def main():
    a=argparse.ArgumentParser();a.add_argument('--output',required=True);args=a.parse_args()
    root=Path(args.output);root.mkdir(parents=True,exist_ok=True)
    if any(root.iterdir()):raise ValueError('fresh owned Qt output required')
    report={'passed':False,'presentation_claim':False,'physical_mac_claim':False}
    try:
        if platform.system()!='Darwin':raise RuntimeError('actual Cocoa macOS required')
        from PySide6.QtWidgets import QApplication,QWidget
        from PySide6.QtGui import QPainter,QColor,QImage
        from PySide6.QtCore import QTimer,qVersion
        if qVersion()!='6.12.0':raise RuntimeError('pinned Qt6.12.0 required')
        # Qt Cocoa WId is its OWN native NSView; require its owning NSWindow.
        objc=ctypes.CDLL(ctypes.util.find_library('objc'))
        objc.sel_registerName.argtypes=[ctypes.c_char_p];objc.sel_registerName.restype=ctypes.c_void_p
        objc.object_getClassName.argtypes=[ctypes.c_void_p];objc.object_getClassName.restype=ctypes.c_char_p
        message=ctypes.cast(objc.objc_msgSend,ctypes.c_void_p).value
        def send(ptr,name,result):return ctypes.CFUNCTYPE(result,ctypes.c_void_p,ctypes.c_void_p)(message)(ptr,objc.sel_registerName(name.encode()))
        spec=importlib.util.spec_from_file_location('owned_parser',Path(__file__).with_name('check-metal-owned-window.py'));parser=importlib.util.module_from_spec(spec);spec.loader.exec_module(parser)
        app=QApplication([]);started=time.monotonic();color=QColor.fromRgbF(.08,.2,.35,1.)
        source=QImage(64,64,QImage.Format_RGBA8888);source.fill(color);raw=bytes(source.constBits())
        assert len(raw)==16384 and raw==bytes([20,51,89,255])*4096
        (root/'raw-source.rgba').write_bytes(raw)
        class Window(QWidget):
            paints=0
            def paintEvent(self,event):
                painter=QPainter(self);painter.fillRect(self.rect(),color);painter.end();self.paints+=1
        widget=Window();widget.setWindowTitle('IngeTrazo own Qt clear diagnostic');widget.resize(800,600);widget.show()
        def finish(error=None):
            if error:report['error']=str(error)
            report['elapsed_seconds']=time.monotonic()-started
            (root/'result.json').write_text(json.dumps(report,indent=2)+'\n')
            app.exit(0 if report['passed'] else 1)
        def capture():
            try:
                if time.monotonic()-started>15:raise RuntimeError('bounded own Qt capture deadline')
                if not widget.isVisible() or not widget.windowHandle().isExposed() or widget.paints<1:return
                view=int(widget.winId());name=objc.object_getClassName(view).decode()
                if 'View' not in name:raise RuntimeError('Qt-owned WId did not identify expected native view')
                native=send(view,'window',ctypes.c_void_p)
                if not native:raise RuntimeError('Qt-owned native view has no window')
                number=send(native,'windowNumber',ctypes.c_long)
                if number<=0:raise RuntimeError('invalid own NSWindow number')
                def object_description(ptr):
                    if not ptr:return 'nil'
                    description=send(ptr,'description',ctypes.c_void_p)
                    if not description:return 'description unavailable'
                    text=send(description,'UTF8String',ctypes.c_char_p)
                    return text.decode(errors='replace')[:2048] if text else 'UTF8 unavailable'
                layer=send(view,'layer',ctypes.c_void_p)
                report['native_window_color_space']=object_description(send(native,'colorSpace',ctypes.c_void_p))
                report['native_view_layer_class']=objc.object_getClassName(layer).decode() if layer else 'nil'
                report['native_view_layer_contents_format']=object_description(send(layer,'contentsFormat',ctypes.c_void_p)) if layer else 'nil'
                report.update(qt=qVersion(),owned_nswindow_id=number,owned_nsview_class=name,actual_QPainter=True,paint_events=widget.paints,raw_source_rgba=[20,51,89,255],raw_source_sha256=hashlib.sha256(raw).hexdigest())
                (root/'own-window.json').write_text(json.dumps(report,indent=2)+'\n')
                cmd=['/usr/sbin/screencapture','-x','-l',str(number),str(root/'own-window.png')]
                run=subprocess.run(cmd,capture_output=True,timeout=min(3.,max(.001,15.-(time.monotonic()-started))))
                (root/'capture.stdout').write_bytes(run.stdout);(root/'capture.stderr').write_bytes(run.stderr);report['capture_command']=cmd;report['capture_status']=run.returncode
                if run.returncode:raise RuntimeError('owned-window capture permission/unsupported refusal')
                image=parser.rgb_png(root/'own-window.png');report['capture']=image
                data=(root/'own-window.png').read_bytes();report['png_sha256']=hashlib.sha256(data).hexdigest()
                metadata=[];offset=8
                while offset+12<=len(data):
                    size=struct.unpack_from('>I',data,offset)[0];kind=data[offset+4:offset+8];body=data[offset+8:offset+8+size]
                    if kind in (b'iCCP',b'sRGB',b'gAMA',b'cHRM'):metadata.append({'chunk':kind.decode(),'bytes':size,'sha256':hashlib.sha256(body).hexdigest()})
                    offset+=size+12
                    if kind==b'IEND':break
                report['png_color_metadata']=metadata
                report['capture_matches_raw_source_bytes']=all(v==[20,51,89] for v in image['rgb'])
                # A managed-display mismatch is retained for attribution, not fitted.
                # Missing/black output cannot be called a diagnostic success.
                if any(v==[0,0,0] for v in image['rgb']) or len({tuple(v) for v in image['rgb']})!=1:raise RuntimeError('owned painted clear unreadable/nonuniform/black')
                report['passed']=True;timer.stop();finish()
            except Exception as e:
                if isinstance(e,subprocess.TimeoutExpired):
                    (root/"capture.stdout").write_bytes(e.stdout or b"");(root/"capture.stderr").write_bytes(e.stderr or b"")
                timer.stop();finish(e)
        timer=QTimer();timer.timeout.connect(capture);timer.start(250)
        code=app.exec();widget.close();return code
    except Exception as error:
        report['error']=str(error)
        (root/'result.json').write_text(json.dumps(report,indent=2)+'\n')
        return 1
if __name__=='__main__':raise SystemExit(main())
