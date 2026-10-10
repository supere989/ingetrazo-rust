# Independent native sampled texture hardware review

Fresh frozen-original Qt/Mesa creates the five literal texture corpus inputs. The frozen native8f14f1b candidate builds in an independent target directory and runs in a separate read-only, network-disabled devbox with CDI NVIDIA device access. Adapter selection is strict NVIDIA/discrete; actual device is RTX5090/API1.4.351/driver615.71.09. No software fallback is accepted.

Actual GPU warm reuse, incomplete-fence refusal, real image-admission budget rollback preserving every old drawable pixel/ledger, successful retry and sampled-image fenced retirement pass the production probe. NativeWorld teardown leaves the ledger empty.

The fresh cross-backend Qt/Mesa versus RTX strict source sample comparison remains FAIL: two cases (translucency/backtexture) match and three (quadrants/repeat/cutout) differ. These are retained facts, not attributed conclusively to one driver or renderer. Actual same-NVIDIA original/native captures and owned-window validation remain required; no tolerance is added. The initial build-container adapter/library setup failures are retained before the separate hardware devbox succeeds.

This five-case candidate also precedes the owner's heldout phase-order correction. Its hardware results do not justify promotion or establish full texture parity. Shaded/XRay, ordering/sections/styles, decoder formats, compression/streaming, native Metal and bounded device-loss cleanup remain open.
