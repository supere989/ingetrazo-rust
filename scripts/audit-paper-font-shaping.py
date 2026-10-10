#!/usr/bin/env python3
"""Engineering-only system HarfBuzz font-function experiment, NOT a product text implementation."""
import ctypes as C,ctypes.util,json,struct,sys,math
from pathlib import Path
hb=C.CDLL(ctypes.util.find_library("harfbuzz"))
def bind(name,args,result):
 f=getattr(hb,name);f.argtypes=args;f.restype=result;return f
P=C.c_void_p;I=C.c_int;U=C.c_uint
blob=bind("hb_blob_create_from_file_or_fail",[C.c_char_p],P);face=bind("hb_face_create",[P,U],P);font=bind("hb_font_create",[P],P);scale=bind("hb_font_set_scale",[P,I,I],None);ot=bind("hb_ot_font_set_funcs",[P],None);sub=bind("hb_font_create_sub_font",[P],P);buffer=bind("hb_buffer_create",[],P);add=bind("hb_buffer_add_utf8",[P,C.c_char_p,I,U,I],None);guess=bind("hb_buffer_guess_segment_properties",[P],None);shape=bind("hb_shape",[P,P,P,U],None);funcs=bind("hb_font_funcs_create",[],P)
CB=C.CFUNCTYPE(I,P,P,U,P);setadvance=bind("hb_font_funcs_set_glyph_h_advance_func",[P,CB,P,P],None);setfunc=bind("hb_font_set_funcs",[P,P,P,P],None)
class Info(C.Structure):_fields_=[("glyph",U),("mask",U),("cluster",U),("a",U),("b",U)]
class Pos(C.Structure):_fields_=[("xa",I),("ya",I),("xo",I),("yo",I),("v",U)]
infos=bind("hb_buffer_get_glyph_infos",[P,C.POINTER(U)],C.POINTER(Info));positions=bind("hb_buffer_get_glyph_positions",[P,C.POINTER(U)],C.POINTER(Pos))
version=bind("hb_version_string",[],C.c_char_p)
source=json.loads(Path(sys.argv[1]).read_text());rows=[]
for case in source["cases"]:
 if case["name"] not in ["latin","accents","bold","italic","underline","serif","mono"]:continue
 call=case["source_draw_calls"][0];path=Path(call["font"]["file"]);b=path.read_bytes();n=struct.unpack_from(">H",b,4)[0];tables={}
 for i in range(n):
  tag,_,off,size=struct.unpack_from(">4sIII",b,12+16*i);tables[tag]=b[off:off+size]
 count=struct.unpack_from(">H",tables[b"hhea"],34)[0];upem=struct.unpack_from(">H",tables[b"head"],18)[0]
 widths=[struct.unpack_from(">H",tables[b"hmtx"],i*4)[0]for i in range(count)]
 parent=font(face(blob(str(path).encode()),0));ot(parent);scale(parent,6400,6400)
 @CB
 def advance(_,__,g,___):return int(widths[min(g,count-1)]*6400/upem)
 ff=funcs();setadvance(ff,advance,None,None);child=sub(parent);setfunc(child,ff,None,None)
 outcomes={}
 for name,f in [("default_scaled",parent),("qt_fixed_advance",child)]:
  buf=buffer();text=call["text"].encode();add(buf,text,len(text),0,len(text));guess(buf);shape(f,buf,None,0);length=U();ii=infos(buf,C.byref(length));pp=positions(buf,C.byref(length));x=0;gs=[];ps=[]
  for i in range(length.value):gs.append(ii[i].glyph);ps.append([(x+pp[i].xo)/64,call["font"]["ascent"]-pp[i].yo/64]);x+=pp[i].xa
  expected=call["supplementary_qtextlayout"]["runs"];assert len(expected)==1
  outcomes[name]={"glyphs":gs,"positions":ps,"exact":gs==expected[0]["glyphs"] and ps==expected[0]["positions"]}
 rows.append({"name":case["name"],"results":outcomes})
Path(sys.argv[2]).write_text(json.dumps({"system_harfbuzz":version().decode(),"scope":"Engineering font-function experiment against supplementary actual Qt runs, NOT original QPainter pixel or native Rust text parity","cases":rows},indent=2)+"\n")
print([(r["name"],{k:v["exact"]for k,v in r["results"].items()})for r in rows])
