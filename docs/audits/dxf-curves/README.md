# Source-faithful DXF curve path subset

Pinned original6be29fe uses ezdxf1.4.4 `make_path`, then `.flattening(max(.01/scale,1e-6),segments=8)`. Circular/elliptical arcs become cubic Beziers with at least one per quarter and tangent4/3*tan(span/4). OCS transforms control points before adaptive flattening. Offset-relative cubic Bernstein evaluation and strict midpoint deviation `<distance` choose dyadic subdivisions. Units multiply only flattened points, before source Qt authored geometry conversion. Tiny bulges<1e-6 become line commands, while original entity curve-selection flags remain separate.

Natural source12workflows/84states: Circle/sagitta subdivision, wrapped/half/OCS ARC, full/partial/tilted ELLIPSE, positive/negative/closed/tiny LWPOLYLINE bulges, legacy closed bulges with OCS/elevation. Existing straight inputs reused only for native Cancel/malformed/f64/opaque guards, no old controller capture. Exact dependency output fixture retains every emitted source Qt point and raw path controls; no tolerance/point normalization. Initial observer failure (Edge uses curve, notcurve_id) preserved.

SPLINE rational/knot/periodic path construction and INSERT/MINSERT remain separate; broader face stitch/fusion stillopen. No P22 promotion, renderer/GPU proof or totalheap claim. New reusable --dxf-curves mode remains within existing contextconsumer. Existing old check-dxf-curves CI sampled test updates only its now-supported OCS expectation; strict newchecker is check-dxf-source-curves.py.

## Frozen result

Runtime `0e1d475b`, checker/reader `be845e39`: onecontextconsumerbuild7.10s andtwoaffectedformatstestbuild5.51s PASS. Independent source-point equality covers every emitted Qt point for all12recipes. Actual App84fullstates match,12original-readerreopens match geometry/schema/curve-ownerinventories,36mutationsreject,3nativeCancel/malformed/f64opaqueguards PASS. The acceptedstraight56states/8reopens remainexact onthisbinary. SHA256 `2eec0f70c24ab641bbc3dfd8aa8b697e8427b24afaa531f0f86726035424e788`. No Rust rebuild after checker-only curve-owner strengthening; actualreader rerun only. `records.zip` pins nativeoutputs, focusedlogs andstraightaffectedJSON; source12recipes capturedonce after retained initialobserver failure.

## ARM087 failure and deterministic math correction

ActualARM087 `lw-positive` finalbulge y was-3.3306690738754696e-16 against frozenLinux source-1.1102230246251565e-16. Failurelog is byte-authenticated fromoriginal CIartifact inportable-mathrecords. `5c4d0cf9` changesonly curveconstruction trig/atan/hypot toalreadylockedlibm0.2.16 withforce-soft-floats; thisoverridesarch/intrinsic choices evenunderCargo featureunification. No point snapping/tolerance orauthored-data change. Coldimport arithmeticonly; no per-framehook orownercloneadded.

All12frozencurvepaths and15splinepaths remainexact onLinux;5focusedtestsPASS. Sourcepositivebulge constructortrace andportableintermediate fields match, includingliteral yresidue. Diagnostic alsoemits currentplatform-math intermediates separately with --nocapture forattribution. ActualcorrectedARM execution remainspending; noCIretry launched. ExistingSPLINEAppbinary wasnotrebuilt afterthismathchange, anditsfrozenproofremainstiedto7fff975c.
