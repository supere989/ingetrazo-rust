# Actual same-runner owned Qt clear diagnostic

The pinned Qt 6.12 QWidget paints the authored QColor on a real Cocoa window. The script captures only that widget’s owning NSWindow ID. A separate raw QImage asserts all 4096 source pixels are RGBA (20,51,89,255). Captured display bytes and PNG color-profile chunk hashes are retained independently; they are not substituted for source bytes.

The native CI step runs on the same runner as the existing strict Metal owned-window diagnostic. Compare their four interior RGB samples and color metadata without rounding or tolerance changes. A successful diagnostic means a real painted, exposed, nonblack uniform owned-window capture was obtained; `capture_matches_raw_source_bytes` can still be false. This does not replace the strict Metal pixel check, positive presentation callback, owner retirement, or 15-second presentation gate.

Unsupported platform, Qt version, native view ownership, capture permission, timeout, invalid PNG, and black/nonuniform output fail and retain result/error metadata. The script changes no production color, window visibility, or ownership code. Actual macOS execution is pending at commit time.
