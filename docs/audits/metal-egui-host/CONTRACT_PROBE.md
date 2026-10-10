# Generic egui Metal sampling contract (native execution pending)

`egui_contract_probe --output=FRESH_DIR` renders ten strips through the production painter and reads every GPU pixel. `scripts/check-metal-egui-contract.py DIR` independently requires the complete 80×8 image and successful protocol.

The pinned epaint 0.30 `FontImage::srgba_pixels(None)` law is coverage^0.55, nearest rounded to 8-bit equal RGBA. Raw coverages 0/.25/.5/.75/1 therefore encode 0/119/174/218/255; black text over white yields 255/136/81/37/0. This is egui's actual font contract, not literal source font antialias parity. Fractional coverage must remain untouched by Color32 inverse-alpha conversion.

Additional strips cover half-alpha black vertices (127 over white), linear midpoint between opaque black/white (128), nearest midpoint (white), nearest repeat at u=1.25 (black), and clamp at u=1.25 (white). Five further actual GPU frames must remain identical without texture or geometry uploads; final charged allocation count must reach zero. Wrong-origin/black/white/empty negatives are independently rejected.

No physical Mac or native execution claim from cross compilation. Arbitrary translucent RGB quantization, interpolated glyph edges, mipmaps, authored CAD, shared application Save/Discard/Cancel and full native world pipelines remain open. The current native host routes actual shared application close requests through DocumentSession but has no automated real-user interaction proof yet.
