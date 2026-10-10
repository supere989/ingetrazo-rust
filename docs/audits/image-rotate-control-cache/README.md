# Rotate render-frame control work

Source cdf9d537 corrects a reviewed hot path: viewport on_control_state calls formerly reposed images or allocated a scene-wide restoration map on every frame. It now returns when copy mode did not transition; interactive restoration reuses the same scalar-epoch UUID map as preview. Read-only history snapshot restoration remains an explicit cold operation.

The real counting allocator protocol initializes 50,000 images and one grabbed image. It asserts zero allocation calls and zero scanned images through 1,000 unchanged noncopy frames, 1,000 varying Ctrl/hover cycles, and 1,000 unchanged copy frames; reversing membership causes exactly one map rebuild and retains grabbed identity. Timing is bounded debug CPU evidence, not GPU performance. Actual original nine/59 gesture states remain exact with literal old and zero-axis negatives unchanged.

Replay cargo run -p ingetrazo-tools --example image_rotate_epoch; rerun image_rotate_tool and check-image-rotate-tools.py against the earlier frozen original/old/zero-axis documents. Tools suite passes. Remaining image rendering and broader transform context scope stays open.
