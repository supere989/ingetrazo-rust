# Physical annotation PDF alpha progress

Qt PDF brush transparency uses QColor alpha8 / 255; the raster painter retains its existing precision. The explicit PDF-only background brush policy reduces radial-fit from 1,089 differing pixels/max-channel-error 2 to four pixels/max1. The entire unchanged frozen corpus still matches 20/20 PNG and 20/20 JPEG, 18/20 PDF, all 20 original-reader pairs and 25 rejecting controls. Cotas_rad-default remains one PDF pixel/max1. Strict acceptance therefore remains FAIL; no tolerance or baseline replacement was introduced.

Executed runtime `a39860ed71735af391bc33d7c0ce6a99f5e34c4c`, binary SHA256 `6afd822d0bc7d37345a8fcc34c5fcfd8982bcb645ee03a80dbae828e1e382322`. Native replay 18.574 s; original reader 0.162 s; checker 1.386 s. The focused source_pdf_brush_alpha_keeps_raster_precision_separate test passes. No new source capture occurred: [the retained dashed-stroke corpus](../dashed-callout-stroke/README.md) was reused. The comparison against that preceding replay changes only radial-fit PDF; every other comparison dictionary is identical.

This packet preserves actual outputs, strict failure, original-reader records, invocation/test receipts and executed Git support bytes. Disposable shader/session data is excluded; nothing was purged. Software Vulkan evidence does not establish representative hardware or native Mac parity.
