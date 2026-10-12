# Rotated annotation rectangle fill

Explicit original rectangle provenance now survives uniform rotation and uses the existing Qt midpoint/ratio rasterizer. The source captures, generic polygon paths, PDF output and GPU blending are unchanged. The shared build took 8.176 seconds; two affected canvas cache tests pass. The first test filter matched zero tests and its receipt is retained alongside the corrected two-test run.

Cotas-position and radial-fit PNG/JPEG outputs now match the frozen source bytes. Independent case-by-case review accepts 14/20 PNG, 14/20 JPEG and 18/20 PDF comparisons, 20 original readers and 25 rejecting controls. Strict annotation parity remains failed. Every other annotation comparison is identical to the alpha baseline; pictures 11/11/11, legends 7/7, symbols 12/12 and captions 13/13 remain exact with unchanged controls.

The archive retains all five terminal reports, current outputs, receipts and logs. Git bindings, binary digests and frozen source archive references are recorded in the manifest. This is software Vulkan evidence; new hardware/Metal execution, full canvas pixels and release gates remain open. Prior failures remain in the alpha and earlier annotation packets.
