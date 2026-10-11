# Additional original NVIDIA shadow controls

Pinned original 6be29fe437a7cd992d4e079f71720821b25216b9; bitmap and mesh face-me controls plus terrain, uploaded nonempty photo mesh, and populated base-map suppression controls. Actual NVIDIA frames and packed depth images, IGZ documents, camera/light matrices, sampler and GL sample queries are in captures.zip. Every restored frame exactly equals its initial frame.

Billboard controls contain an ordinary loose edge bounding (-1,-1,0) to (1,1,3), hidden by the edge-off style. This is necessary because original Scene.bounds excludes truthy billboard groups. The edge supplies bounds, not face casters. Bitmap casts; mesh face-me does not.

Both fixture failures are retained. The first had only billboard geometry and therefore no original scene bounds/shadow map. The second used an invalid bitmap billboard without a texture as a supposed vector-art control, which crashes the original QImage(None) path. That invalid control was removed, not treated as vector parity evidence. Vector-art behavior remains unverified.

Original source is unmodified. The fixed-sun override and observation hooks are recorded in the oracle script. Native comparison is a separate acceptance.
