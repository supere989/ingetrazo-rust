# Read-only own-window color metadata candidate

Paired actual Intel evidence records GPU drawable RGBA20/51/89/255 and actualQt raw20/51/89/255, while the owned Metal screenshot is21/51/89. Both PNG ICC profiles are byte-identical. This observation localizes the mismatch after GPU production without assigning a particular compositor cause.

The next diagnostic records actual own NSWindow.colorSpace descriptions for Qt and Metal. Metal additionally records CGColorSpace name and layer.contentsFormat; Qt records its actual NSView layer class/contentsFormat when present. All queries are own-window only. No property, palette, shader, raw reference, capture tolerance, presentation callback rule or15-second deadline is changed.

Both architecture all-target cross checks pass. Actual native evidence is still required. Prior strict capture and ARM callback failures remain retained; metadata alone cannot establish a rendering or ownership fix.

Primary source Qt6.12 QCocoaBackingStore::colorSpace() obtains the QNSView.colorSpace.CGColorSpace, so the source observation also records that effective view property: https://raw.githubusercontent.com/qt/qtbase/v6.12.0/src/plugins/platforms/cocoa/qcocoabackingstore.mm . This additional observation does not change the source painter or color policy.
