# Source solid-brush premultiplication (bounded)

`1ddc6d0` preserves the straight EncodedRgba API. A native palette is prepared once when its immutable Arc changes, following Qt6.12 QRgba64::premultiplied and rounded div257 to the eight-bit raster target. Coverage is applied afterward in the fragment shader. No Color32 conversion, palette fitting, extra image/pass, or warm geometry upload is introduced.

The primary source is [Qt6.12 qrgba64.h](https://github.com/qt/qtbase/blob/6.12/src/gui/painting/qrgba64.h). The QRgb32 div255 shortcut initially seemed sufficient; actual QPainter disproved it at24 of65536 channel/alpha pairs, retained in original.json and the rejected test log. QRgba64 conversion matches actual QPainter solid brushes at every pair. The protocol example calls the same production operation used by palette preparation. The actual original reference uses QPainter.fillRect, not a copied formula.

Fresh native software application startup, two resizes and OS close retain precise dark RGBA(1,7,19,128) over white [128,131,137] and original RGBA(37,93,157,128) [146,174,206]. Each run has one72-byte geometry upload. Independent actual RTX prior dark failure remains321f148/86fa956; its corrected retest is pending and is not replaced by software proof. AA coverage/composition, full authored shape geometry and Metal remain open.

```
python scripts/reference-qt-cad2d-premultiply.py ORIGINAL
cargo run -p ingetrazo-render --example cad2d_premultiply_protocol -- NATIVE.bin
python scripts/check-cad2d-premultiply.py ORIGINAL NATIVE.bin --output RESULT.json
```
