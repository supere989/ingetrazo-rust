# Independent Fillet volume

The standalone checker reads authenticated retained source/native face coordinates, never native `Mesh.volume` or the fillet kernel. Six literal case identities are required. The two invalid old oblique/turned authoring recipes are replaced only by the existing authenticated corrected original archive; the invalid oblique model remains a rejected negative.

It finds face components by boundary edges and requires one outward closed component with exactly two opposite uses of every edge. Loose wires do not contribute. The explicitly open neighboring face is reported and excluded. `interior` is metadata and does not remove an actual boundary face. Oriented polygon fans integrate signed volume exactly with binary-coordinate rational determinants; signed integration is valid for planar concave loops and oppositely oriented hole cycles. Already accepted source/topology proofs remain the authority for geometric validity beyond incidence.

A source-to-native comparison accounts only for the source's actual binary32 coordinate rounding cells, propagating exact rational intervals through the volume polynomial. No measured difference, percent bound, or fitted tolerance chooses those cells. Translation cancellation is reduced by an exact fixed origin. The turned model's conservative interval is wider because it has182 faces; its independently measured source/native discrepancy is1.5888e-7 cubic units. This check supplements, rather than replaces, the strict existing topology/coordinate/metadata comparator.

For the2×1×1 box, radius r=.2, N=8 subdivisions, selected edge length L=2: the removed corner cross section is r² minus the discrete quarter-circle polygon sector N*r²*sin(pi/(2N))/2. Thus V=abc−L*r²*(1−N*sin(pi/(2N))/2)=1.982428903045161. Pi and sine are independently computed at90 decimal digits using Machin's identity/Taylor series. This analytic value falls within the native binary64 coordinate-rounding-cell integral, approximately[1.9824289030451603,1.982428903045162]. The source value is1.9824289020188646, consistent with its Qt coordinate cells.

Five negatives reject: topology-preserving homothety changing volume, vertex displacement changing volume, reversed boundary, missing face, and old invalid-registry oblique output. No new source capture, Rust build, UI run or acceptance scope was introduced.

Replay:

```sh
python3 scripts/check-fillet-independent-volume.py docs/audits report.json
```

This closes the independently checked numeric-volume gap for these retained P09 cases. Full-criterion disposition belongs to combined review with the existing radius/segment/topology/history proofs.
