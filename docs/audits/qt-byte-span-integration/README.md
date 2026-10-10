# Reviewed Qt byte-span integration

The production inline CAD shader now rounds premultiplied RGBA after eight-bit raster-span multiplication using the pinned Qt source law. Fresh actual Qt exhaustive DestinationIn validates 65,536 channel/coverage pairs. Independent RTX replay matches the actual black AA mask and all three colored spans, plus ten full-coverage colors, across two viewport sizes. Rebuilt pinned shaderc SPIR-V is byte-identical. This adds no render pass, copied image or process.

Diagnostic gradient interpolation remains failed and separate from raster-span coverage. All nine actual production shape images still fail strict whole-image equality; retained independent verdict and unchanged original images are in ../composer-byte-span-production/. This arithmetic correction does not establish full shape antialiasing or any completed parity row.
