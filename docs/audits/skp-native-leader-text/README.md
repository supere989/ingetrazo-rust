# Positive native SKP leader-text records

Three actual pinned OpenSKP SkpBuilder MFC v17 archives exercise Unicode/trimmed leader text, zero leader offset and skipped empty body. The pinned original application imports each archive independently; native Rust imports each and the original IGZ reader reopens its output. All recursive group/material/layer/sharing and explicit imported field inventories match exactly, including positive text anchor and offset plus a dimension. Altered geometry is rejected. These are actual writer-authored records, distinct from the real-model gondola/capilla/v7/v2014 evidence; they do not establish screen-label or rendered leader parity.

The native binary is the unchanged production21a2a70b executable SHA9108cb56144425782e823302f1720418f4810c83638bb7351711893efef13544. The report source8b2cd775 adds only the metadata save test. Both oracle pins are asserted. Replay scripts/check-native-skp-leader-text.py ORACLE BINARY OUTPUT.
