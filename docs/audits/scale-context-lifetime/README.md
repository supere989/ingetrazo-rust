# Scale context lifetime

This five-recipe, 23-phase consumer is deliberately not a full literal parity pass: 17 phases match exactly and six retain known source/native differences. The checker authenticates exact inventory and fails any additional drift. Five native whole-session Cancel invariants and five corruption mutations pass separately.

Actual pinned `Viewport.begin_group_edit`/`_leave_group_edit` are called on a real Qt QWidget with a QMainWindow parent, with Scale's actual handlers and History. Integer-translated parent/child placements exercise world context coordinates. Native calls actual app context and tool dispatch methods. No visible UI, pointer entry, or GPU claim is made.

Source `groups` is the document tree during editing. The comparator's `context_groups` observation selects the current editable group's children (none at depth two; outer children at depth one), matching native's explicit active-context collection. The original full groups remain untouched in source.json. No geometry, transforms, flags or metadata are normalized.

Committed nested Scale, folded Finish, refused hot retype after the share-back command changes the history head, Undo and Redo all match. Image-only preview remains global across Finish and cancels correctly. Native whole-session Cancel has no original counterpart and restores both parent placement and global image frames.

Source geometry-preview Finish publishes scaled geometry with a share-back history command but leaves Scale's captured detached world vertices active; later Scale Cancel does not cancel the published shape. Mixed geometry/image preview similarly publishes geometry while Cancel later restores only images. Native intentionally retires/restores geometry previews at the boundary; this integrity behavior remains a literal source divergence.

Source entry while a group preview is active bakes its preview transform into children, then Scale Cancel restores the original parent transform onto those already world-baked children. The retained raw result demonstrates the wrong world placement. Native cancels the transient preview before entry and preserves original geometry/placement. This is an explicit integrity safeguard, not a parity claim.

The first draft of the source recipe passed integer 2 rather than float 2.0 to on_value; original correctly refused the integer under its float-only internal handler contract. The final script uses the actual parsed-value float domain. This was fixture correction, not product behavior correction.

Replay in ingetrazo-publish:

```sh
CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo build -j2 -p ingetrazo-gui --example scale_context
/tmp/ingetrazo-metal-world-target/debug/examples/scale_context docs/audits/scale-context-lifetime/source.json /tmp/scale-context-native.json
python3 scripts/check-scale-context.py docs/audits/scale-context-lifetime/source.json /tmp/scale-context-native.json
```

No runtime changes were necessary for the matched paths. Actual pointer context entry/exit, post-boundary image commit/retype, rapid modifier taps, arbitrary context types and full rendered lifetime remain outside this bounded proof.
