Actual pre-fix UI alpha failure

53bbca3 sends gamma-premultiplied Color32 bytes to an encoded-premultiplied blend path. The actual NVIDIA UI fixture retains 17 failed strict comparisons out of 24 against actual QPainter, including solid alpha, managed texture alpha and tint, and partial alpha update. Opaque controls pass. This is a failure baseline, not acceptance. The source/native RGBA samples and valid 800×600 images are retained. No desktop image was captured.
