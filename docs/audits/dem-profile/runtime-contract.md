# Native DEM Profile runtime contract

Prereview candidate; no Rust execution yet. Original corpus is the authenticated five-case / thirty-state source-records.zip, pin 6be29fe. Production remains AWS Terrarium zoom13/max15. Native controls use separate runtime-provider.json and owned loopback HTTP; acceptance never contacts AWS.

Profile chooses visible PhotoMesh first, otherwise DEM. Selection becomes the held path; arrivals update that held path independently of current selection and document/history revisions. CSV samples the held path afresh. DEM heights are absolute: datum altitude is not added. Viewport vertical-reference comments remain independent of sampler choice.

Explicit cold Profile/CSV may read cache once per distinct station tile per action. Workers request the entire source path bbox through a constant-memory cursor/bounded batches, including off-path tiles. Warm repaint only polls decoded fields and pumps queues, without disk IO/decode/hash/scene copies. Existing cached chart still copies its Mesh for egui0.30's frame-owned Shape handoff.

Service publication follows valid action/result admission. Eight persistent workers start only for missing queued tiles and remain at App lifetime; panel/document changes never join them. Replacement retires publication while an already-running old-source response may finish decoding/persisting its old cache, matching the frozen source API. Retired queued work is skipped; no claim that every abandoned original queue drains.

Profile CPU budget includes encoded/decoded reservations for eight active/eight queued/one polled owner, bounded control metadata, disk index, accepted old owners and new fields/profile/chart. Image-library headers/TLS/worker stacks/process heap are not claimed as exact physical charges. Disk LRU separately follows source256MiB. Index/pending directory charges are conservative bounded control accounting. No hypothetical whole-bbox allocation/cap.

ureq2.12.1 validates HTTPS; gzip disabled. Redirects reject HTTPS downgrade. Total transfer/body deadline and byte caps are native guards; OS DNS may exceed the deadline. App Drop closes send/receive before joining workers; DNS can delay destruction. No detached workers/process resolver or claimed DNS deadline.

Replay: run-dem-profile.py BINARY SOURCE; reference-dem-profile-reopens.py SOURCE NATIVE; check-dem-profile.py SOURCE NATIVE COMPARISON. Native context_document_lifecycle SOURCE --dem-profile. Original reader pairs use native IGZ plus native disk cache and explicit reopened selection; final switch uses external new-provider API state.

Guards cover read-only document/history, retained accepted owners/charges on CPU refusal, warm reuse, retired-source response isolation, corrupt refetch and off-path arrivals. Focused loopback body-stall/cache overwrite/LRU test included. All numerical/runtime results await the single reviewed root build. Reuse nine frozen PhotoMesh inputs once; its hidden-photo guard now uses operative DEM with an explicitly offline provider.
