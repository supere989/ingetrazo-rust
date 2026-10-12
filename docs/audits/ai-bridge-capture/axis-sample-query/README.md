# Actual source sample-position query

The retained source Qt/OpenGL scene FBO reports four samples at (0.375, 0.125), (0.875, 0.375), (0.125, 0.625), and (0.625, 0.875), matching Vulkan standard four-sample locations in the current bottom-origin world target. Line width is 1, multisampling is enabled, and line smoothing is disabled. The query returned zero in 1.917 seconds.

This diagnostic uses a fresh default window (795 × 711), not a frozen capture pose. It reads the actual scene FBO and restores the binding; it captures no images and changes no golden corpus. It rules out a sample-position mismatch on this software driver, but proves neither exact axis rasterization nor hardware or Metal parity. The next diagnostic compares actual axis vertex bytes and uploaded MVP at the declared frozen pose.

`records.zip` retains the exact query script, result, execution receipt, output and Xvfb log. `manifest.json` authenticates every member and the archive. The script hash agrees with the execution receipt. A Qt texture destruction warning at process exit is retained in stderr; the query completed successfully.
