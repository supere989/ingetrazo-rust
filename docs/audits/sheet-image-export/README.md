# Sheet image export workstream

Seven actual original PNG/JPEG exports cover physical sizes, orientation, resolution, shape layering/hidden items and text. Six original handler controls establish cancellation, extension selection, remembered DPI and failed-write ordering. Dialog answers and frame-refresh counters are supplied controls, not desktop dialog or frame-rendering proof. Source images and metadata are frozen once.

The native codec consumes opaque RGBA8 directly: PNG streams one RGB row; JPEG borrows the RGBA pixels without a second full-page copy. Dimensions use source nearest-even rounding and physical density metadata. Atomic publication preserves destinations on failure. Codec acceptance is pending; native GPU page preparation, tiled export and GUI integration remain in progress. No P29 promotion or native image-rendering claim.
