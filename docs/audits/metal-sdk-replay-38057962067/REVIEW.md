# Actual native SDK replay after packaging

Source8c540f7316e31d1fc9a5c78d3a2e0db86f73a0d9 run38057962067 ARMjob114230193413 artifact11671969481.
Combined GUI --all-targets check now succeeds with actual Apple SDK (53.91s). Next build command fails inventory: no example target named native_metal. No linker/shader error or application presentation acceptance. New direct native_metal example invokes existing shared host; previous failed workflow source/log preserved. Full actual example SDK linking replay still required.
