# Legacy straight DXF POLYLINE subset

Authority: unchanged original `formats/dxf_in.py` at6be29fe. Actual SnapshotImport + Viewport contexts produce8workflows/56states. Covers straight open/closed2D/3D, elevation and OCS reversed/oblique; nested import stays document-root without finishing active edit, then chronological history and Finish. Geometry observer retains ordered group inventory and full points/edges/faces; no ring tolerance/coordinate fit.

Runtime consumes POLYLINE/VERTEX/SEQEND once into existing DxfPlan. New source-import points alone narrow to Qt float; old direct f64 callers remain f64. OCS uses literal arbitrary-axis algorithm. MissingSEQEND and nonfinite output refuse before publication. Cancel/invalid transaction and protected nativef64/opaque history guards are separate from natural source cases.

Bulged legacy polylines remain explicitly refused; ELLIPSE/SPLINE/INSERT/MINSERT, existingcurve ezdxf-flattening differences and source face stitch/fusion remain open. No P22 promotion, GPU or totalheap claim. No accepted OBJ/MTL/nested source recapture.
