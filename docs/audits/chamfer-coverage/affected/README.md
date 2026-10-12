# Affected physical text-export checks

The existing frozen caption, legend, annotation and picture exports were replayed after the selective glyph clipping repair, with no source recapture and at most two concurrent consumers. Captions pass all thirteen PNG/PDF pairs and eleven controls. Legends, annotations and pictures retain strict failures, with every per-case source/native output hash and residual metric identical to their retained baseline. The independent classification and baseline references are in [affected review](../affected-review.json).

The archive preserves current native outputs, reports, logs and receipts, plus the passing single/multi-mask glyph wiring test. No renderer performance, full prefix parity or release-gate closure is claimed.
