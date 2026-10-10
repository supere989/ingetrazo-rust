# Source CoreText synthesis correction

Qt6.12 uses a float14-degree tangent for native synthetic italic. FreeType's existing0.3 policy remains unchanged. Native boundingBox combines raw CGRect width+height*shear before QFixed conversion; device scaling is quantized afterwards. The candidate aligns both operations for native fonts only. It does not add synthetic-bold Canvas support or claim complete raster/layout/PDF parity.

Focused extracted production scalar test passes at a fractional native-metric control. The initial control at a smaller scale made both old/new formulas quantize to the same output and could not distinguish them; changed to unit scale where the old ordering fails. This is scalar source policy evidence, not native glyph or pixel evidence. No broadbuild/cloud. SourceURL/hash retained; SDK/pixel replay must use actual source controls.

## Operative PDF counterexample and correction

Actual coordinated run38085609658/source0464623b Times bold-italic PDF pages are exact at72/150/300DPI with the existing0.3 PDF synthesis matrix. The earlier inference applied CoreText's14-degree alpha-map policy through a method shared by PDF and Canvas; this was too broad. The followup preserves CachedFont::synthetic_italic_shear's0.3 (the direct PDF writer consumer) and computes native14-degree shear only inside CachedFont::rasterize_glyph's CoreText branch. Generic FreeType raster behavior remains unchanged.

Call-path review: sheet_pdf uses synthetic_italic_shear to write its PDF text transform; composer_text_canvas routes through CachedFont::rasterize_glyph, which now selects nativeCoreText shear separately. Native CGRect-width QFixed order correction still applies only to raster. The exact TimesPDF counterexample remains in root coretext-native-third/arm.zip; no repeated PDF run or broader claim. Actual Canvas glyph pixel proof remains pending.
