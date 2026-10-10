# Retained opaque strip admission failure

Frozen production 4d10a1fc on actual NVIDIA RTX 5090 WSI, exact integer CAD2D enabled, exited before first capture. The new original OpenGL strip geometry was refused because it lacks disjoint raster-span metadata. The fixture reports the original error and bounded cleanup committed_bytes=0; the outer owned-window readiness timeout is a consequence. No image comparison or visual acceptance occurred.

Followup derives opaque admission from immutable exact coverage1 vertices and validated exact alpha1 palette, skips destination reads in the integer fragment shader, and preserves normal batch ordering. All other geometry still requires the existing ordered disjoint span proof. No extra renderer/pass/image copy or caller assertion flag.
