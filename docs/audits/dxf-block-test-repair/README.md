# DXF block test fixture repair

Publication `cc7468ff` reached the same obsolete assertion on Linux and both native Macs: the older transaction test changed LWPOLYLINE to INSERT and expected rejection. INSERT is now implemented. A missing block definition is skipped while an ordinary valid LINE remains, consistent with the retained source block workflows. Production behavior is unchanged.

The existing test now places a non-finite Y coordinate in the polyline after the valid LINE. It requires the exact `Invalid polyline field` parse error and retains the whole-scene equality assertion. Independent review checked the parser path and transaction boundary before execution. The named test passed using the existing target in 6.828 seconds; [result](result.json) binds the changed source digest and [log](test.log) retains the invocation output. No broad local suite or source workflow was repeated.

The final strict symbol CI step now requires an explicit readiness output emitted after its combined build. It can still run after subsequent semantic failures, but a skipped or failed build no longer creates a misleading missing-executable comparison failure. Independent review checked this gate; strict acceptance remains unchanged.

[First publication failure](../native-platform/cc7468ff/README.md) remains retained. A local repaired test is not a replacement for subsequent native platform acceptance. Publication replay will accompany the next coherent runtime batch.
