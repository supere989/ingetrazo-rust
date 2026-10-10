# Image-plane command ownership: bounded source proof

Source 0aafeb63, pinned Python 6be29fe437a7cd992d4e079f71720821b25216b9. Four actual original viewport command sequences and production Rust app-controller sequences compare 56 complete states including selection, layers, history counts and reference frame data. Ordinary/folded and nested contexts are covered. Original archive extraction paths are normalized to the literal scan basename; separate actual original reopen verifies every surviving embedded scan SHA256 equals original bytes.

Workspace: 548 passed, 0 failed, 2 ignored. Metadata/asset regression deletes the first plane, edits surviving plane defaults and retains unknown fields and actual archive bytes. Missing modeled runtime baseline and earlier empty-asset/default-layer-alias failures are retained. Explicit Reference layer avoids original save's one-default-layer omission in the authored protocol.

Open: GPU plane rendering/picking, interactive transform tool dispatch, rotate/scale/absolute transform command families, Add-created-layer dynamic name ownership, global layer deletion, detached command behavior, history branch cache reclamation, malformed/exotic codec fields and scan decode/memory budgets. ImageObjects currently retains held metadata until Scene owners drop; no runtime memory lifetime closure. This does not close image-plane parity.
