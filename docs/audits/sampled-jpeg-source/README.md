# Source JPEG sampling parity

Runtime `f5b00b765587b0be5a89f5c4d926b2fbf76c2013` routes sampled JPEG textures through the existing source integer IDCT/fancy upsampling decoder. PNG keeps its prior path. Output admission precedes decode; the decoder also checks its work bound. Caller-owned encoded bytes and total process memory are outside that bound.

Actual Qt 6.12 controls improve from 10/32 exact (5,846 differing channels) to 32/32 exact, including grayscale, progressive and odd dimensions. All four JPEG assets extracted from Chapel also match actual Qt bytes exactly. `check-source-jpeg-decode.py BINARY OUT` drives `native_texture_decode_protocol`; retained reports pin original source and binaries. Fixtures and negative baseline remain compressed here.

The actual own-window Chapel capture remains a strict failure: 1,066 annotation-on/restored pixels and 1,113 off pixels differ, maximum channel error 37. All 3,830 overlay-union pixels remain exact and native restore is exact. This codec fix does not establish full-model image improvement. Original frames are retained at `../annotation-widget-neutral-policy/capilla_quiroz_v17-iso/original`; fresh native PNG/transport evidence is in `widget-capture.zip`.
