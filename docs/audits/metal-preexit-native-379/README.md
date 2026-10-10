# Actual pre-exit native ARM failure

Run 38058554858, source 379a69ac8ed1cbb87f0d9118d4b73b20e1e46cb2, job 114231928442, artifact 11672611345, declared archive SHA256 0278069d3d755197d2afd3b50ab42e5e0d7ec40447341ac3d6c7df59105bd8b7. Complete downloaded artifact retained.

All three actual application choices remain phase 0 with no injected close/mouse events. Crucially, the new snapshot is before event-loop exit: window visible true, layer hidden false, valid extent and drawable size. Each retains three completed commands with positive GPU time but no terminal presentation callback. Save/cancel submit 5, retire 2 skipped; Discard submits 7, retires 4 skipped. Displayed remains zero. This refutes a hidden-window explanation for these actual failures and preserves required unknown owners. No visibility correction or deadline change is justified by this evidence.

Generic egui/font/mip/compression/clear steps passed. Actual native presentation/owned capture/lifecycle failed. Apple Paravirtual proof is not physical Mac acceptance. Intel same-run result remains pending separately.
