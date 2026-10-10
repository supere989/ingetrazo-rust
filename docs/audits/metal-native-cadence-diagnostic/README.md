# Independent native display cadence diagnostic

Apple documents CAMetalLayer.displaySyncEnabled=true as vsync synchronization, false as immediate native presentation potentially tearing. This separate engineering probe compares that one cold selection, not colors, drawables, render contents, or retirement criteria. Default production `attach` remains synchronized true. Probe-only `--diagnostic-no-vsync` requests false through a direct native constructor. No per-frame environment access or wrapper is added.

The existing synchronized presentation/resize gate remains independent and unchanged. The additional process uses the same exact positive callback timestamps, real GPU completion, source owner retention, requested resize, six positive display requirement, and 15-second deadline. It does not count skipped callbacks or GPUCompleted as display. No native success or causal claim at source commit time; both Mac architecture all-target crosschecks passed.

The actual ARM visible-content/callback failures remain retained. A false-mode pass could distinguish cadence scheduling but would not justify silent default change or prove physicalMac/backend completion.

Official source: https://developer.apple.com/documentation/quartzcore/cametallayer/displaysyncenabled
