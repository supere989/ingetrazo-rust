# Full application clip admission

Actual native lifecycle run 38050590186 fails all three choices on both Mac runners in phase zero: no input events, no geometry uploads and no presentation submissions. Logs repeatedly report Invalid egui mesh geometry/clip. Full downloaded archives and six literal failure results are retained with GitHub digest provenance. This is not evidence of a Cocoa button-delivery failure.

The old admission rejects all nonfinite clip rectangles, including egui Rect::EVERYTHING. Native clipping now permits non-NaN unbounded clips and intersects their coordinates with the actual physical target before integer scissor conversion. Geometry indices, finite position/UV and shader domain validation are unchanged. Empty target/domain and NaN clips still refuse. Empty/reversed clips produce no draw. Detailed bounded geometry/clip diagnostics distinguish any other actual offender on the next run.

A literal unit control records the old refusal of Rect::EVERYTHING, exact full-target/new bounded subset scissors, empty clip and malformed NaN refusal. Both Mac tests crosscompile. This is a candidate correction and diagnostic, not an actual lifecycle pass or proven sole cause: fresh native execution is required. The original 15-second deadline and positive modal presentation identity checks remain unchanged.
