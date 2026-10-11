# Three-point section stage observation contract

Diagnostic only. Base observation source 7ac1ca0d; authenticated terminal evidence dddfea0f. No bias, shader depth, geometry, sample pattern or resolve policy changes.

## Fixed inventory and guards

Only closed-iso and top-origin points (297,329), (297,328), (298,303), four ordered samples each. Ordinary and fully observed final RGBA must both equal the retained ARM frame SHA256 9fabf645e1f3518902a2cfebbbae600663d48e8caf3980391d44a12313e88e55 (1,920,000 bytes). Original full frame must equal the retained source closed-iso PNG. Camera bits, sample locations, actual depth format, source pin and draw inventory remain mandatory. Prefix images have no final-image parity claim.

## Stage bindings

1. before-default-back: immediately before the first non-overlay MaterialKind::DefaultBack operation; source immediately before the default-back conditional in views/viewport.py around1696.
2. after-default-back: after the last contiguous non-overlay default-back operation and before the next operation; source after default-back state cleanup and before _draw_instanced_faces around1740. Admit the literal closed fixture only: one expanded opaque stream, one default-back stream, no instanced/texture/alpha/overlay operations. Refuse another inventory rather than guessing equivalent boundaries.
3. after-mask/before-cap: encode_fill after all mask draws and before switching to cap depth/pipeline; source _draw_section_fill after mask draw/state cleanup and before the cap glDrawArrays. Mask writes stencil only: depth/color must equal the pre-fill prefix; this is a required invariant.
4. after-cap: after the cap draw, before contours; source after _draw_section_fill returns (call around1780). Complete fixture excludes shadow/alpha passes between the after-back and pre-fill stages; assert that scope.
5. final: existing post-contour read-only observer, before production tile resolve.

## Native in-pass color method

Extend the existing read-only tile observer output to five disjoint 192-byte stage slices. Bind byte offset stage*192 using setTileBuffer_offset_atIndex; points remain the existing constant uint2[3] binding. The kernel never writes imageblock/depth/stencil. Invoke stages1-4 at the boundaries above without ending the render pass. Each subsequent draw reinstalls its existing render pipeline (material loop already does so; cap explicitly does so). The tile pipeline changes no vertex/fragment buffers, cull, winding, bias, stencil reference or depth state. Final normal/observed whole-frame equality detects any perturbation, and all slices begin as NaN sentinels and require complete finite writes.

## Depth method: terminal prefixes, explicitly not split-pass continuation

The current imageblock observer has no depth access. Do not end/resume the production pass to read depth. Replay the same Prepared in a separate command, with the same ordinary pass load/clear/actions and original draw sequence up to one requested stage, then terminate encoding permanently. No draw resumes after the readback. A private probe-only stop enum propagates from the material loop/fill stage to encode_inner; the caller runs the existing production tile resolve for prefix color, ends the pass, and performs the existing same-format multisample depth compute read. Depth store is diagnostic-only as already admitted; command/Prepared/buffers/pipelines retain the existing Completed/quarantine ownership. Record these depths as terminal-prefix measurements, not in-place observations or proof that a split pass matches production. Five-stage in-pass colors must equal corresponding prefix colors at all12 slots; full observed final image must remain exact. A mismatch invalidates the prefix attribution.

## Original bindings

Pinned Python files remain untouched. Compile a probe-local AST copy of the actual scene paint method/_draw_section_fill, preserving every statement through the requested boundary and inserting only a terminal return there. Record source function SHA256 and exact hook AST path/count. Do not reconstruct source draw commands. Each prefix uses a fresh bounded actual frame render with the same viewport/camera/scene; after it terminates, use the already reviewed same-format/same-sample-count renderbuffer copy and texelFetch readback. A separate unmodified full render must match the frozen source frame before any prefix output is accepted. No prefix is continued. Query actual source depth/stencil format and raw sampled normalized depth bits. Stencil observation, if supported by the existing attachment, is supplementary; cap color transition alone does not prove whether rejection was stencil or depth.

## Interpretation and numerical controls

Keep units/scales unchanged: ordinary/default-back(1,1), mask(0,0), cap(2,2). Source Mesa25.2.8 UNORM setup uses (units+.5)/(2^24-1)+scale*max(abs(dzdx),abs(dzdy)); this is not a fitted two-ULP correction. The bad sample3 first changes front->default-back only if the later contender passes; unchanged neighbor distinguishes a blanket depth shift from ownership. At the cap point, before-cap versus after-cap determines whether the magenta sample was actually written. Final front depth cannot be compared directly with a source cap depth as a same-surface bias measurement. Do not infer rejected contender depth from the retained depth attachment: it records the winner only. If rejection remains ambiguous, propose a separate contender-only control afterward, not extra writes in this run.

No build, original capture or cloud run is part of this prereview commit. Production paths pass None and retain identical draw/store/resolve behavior.
