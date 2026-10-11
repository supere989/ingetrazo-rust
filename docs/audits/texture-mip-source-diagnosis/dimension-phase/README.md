# Unresolved original mip dimension phase

Actual pinned Qt/OpenGL viewport texture creation on the RTX 5090 was observed over 256 even-width/odd-height ramps. Every encoded input and every mip byte is retained and hash verified. All 256 base levels exactly match mirrored Qt RGBA decoding. The first generated mip pixel alternates between red 0 and 11 according to image dimensions, although horizontal reduction is 2:1. This isolates a generated-mip phase difference, rather than encoded-image decoding.

The archive also retains a standalone NVIDIA GL compute arithmetic observer. It is not the original mip implementation. Simple reciprocal/product hypotheses failed to explain the complete matrix; no dimension lookup, fitted coordinate offset, or widened comparison tolerance was introduced. The area-mip candidate remains isolated and full model parity remains open.

A [2019 firsthand NVIDIA forum report](https://forums.developer.nvidia.com/t/anyone-see-a-pattern-51-59-63-83-87-95-99-101-111-117-123-125-135-143-147-glgeneratetexturemipmap-spikes/72467) describes a similar dimension-dependent shift. That report does not establish a confirmed driver defect or a fix.

This is a retained diagnostic checkpoint. Further speculative arithmetic probes are deferred while implementation proceeds on missing application consumers.
