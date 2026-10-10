# Source CoreText synthesis correction

Qt6.12 uses a float14-degree tangent for native synthetic italic. FreeType's existing0.3 policy remains unchanged. Native boundingBox combines raw CGRect width+height*shear before QFixed conversion; device scaling is quantized afterwards. The candidate aligns both operations for native fonts only. It does not add synthetic-bold Canvas support or claim complete raster/layout/PDF parity.

Focused extracted production scalar test passes at a fractional native-metric control. The initial control at a smaller scale made both old/new formulas quantize to the same output and could not distinguish them; changed to unit scale where the old ordering fails. This is scalar source policy evidence, not native glyph or pixel evidence. No broadbuild/cloud. SourceURL/hash retained; SDK/pixel replay must use actual source controls.
