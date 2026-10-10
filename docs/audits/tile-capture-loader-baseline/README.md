# Actual geographic map-block loader failure baseline

Actual pinned original IGZ loader opens all twelve documents. Missing map fields use source defaults; shipped provider identities override authored preset metadata; numeric strings/bools, opacity clamping, empty patches and custom providers are interpreted by TileLayer.from_dict. Invalid zoom/radius/patch data costs only the map, not the file. An empty block is ignored. Zoom300 remains accepted metadata at file-open boundary.

Current geographic candidate actual native IGZ loader differs on all twelve inputs; required serde fields and narrowed zoom reject whole documents. These failures are retained before any production correction. Actual fetch/quad/GPU controls, exotic field representations and physical memory budgets remain separately required full scope.
