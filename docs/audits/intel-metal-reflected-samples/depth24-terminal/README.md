# Terminal Depth24 diagnostic failures and refusal

Three completed Intel Mac diagnostic runs are retained with their exact artifacts and terminal run/job/artifact APIs. All artifact digests, nested members, source Git bindings and the final binary digest authenticate. The first two ZIPs were reused; only the missing final artifact was downloaded. No build, native consumer, checker or original capture was executed while packaging.

Run 38276048523 at `af1d6226` failed to compile the GUI example because the Metal device protocol method was not in scope. Native execution and the checker were skipped. Run 38278045013 at `801409f8` retained the attempted direct trait-import failure: unresolved `objc2_metal` import plus the missing method. Native execution and the checker were again skipped. Successful root metadata checks are separate and did not establish these GUI example builds.

Run 38280805920 at `2b956161` built the actual diagnostic consumer successfully. Its native process exited one with `Native Metal device does not support Depth24UnormStencil8`. The untouched default reference rendering produced `native/closed.rgba` before the explicit candidate-capability guard; that file is not a Depth24 candidate result or source-parity pass. The experimental Depth24 rendering and strict comparison did not execute. The final binary digest and all source/step bindings are retained.

The refusal establishes only that this hosted device does not advertise the required native format. It does not establish a property of all Intel hardware, an Apple CGL depth selector, or source-format parity. Production admission remains unchanged and the existing reflected D32 diagnostic's strict pixel/raw-bit failure remains open. There is no CPU fallback, backend wrapper, fitted depth offset, tolerance or extra row-copy pass.

The packet preserves compiler failures and capability refusal as different outcomes. Missing/skipped checker reports are never promoted to acceptance; no tracker or release gate is changed.
