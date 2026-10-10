# Empty entity layer labels

Frozen original core.layers.entity_layer uses the label or DEFAULT_LAYER. Operative original Scene.entity_visible produces [false,false,true,false,true] for None, empty, Layer0, Layer 0, and a single space when the exact default is hidden and authored empty/Layer0/space registry entries are visible. Original JSON is retained; reference-paper-default-layer.py reproduces this assertion.

Native focused Scene and saved-paper tests verify empty labels use the hidden exact default while authored registry names remain unchanged. Native image admission and cached outline filtering use the same helper; image picking already normalized empty labels. No new rendered-pixel or complete layer scope claim. Existing nonempty/absent label rendering evidence is unchanged.
