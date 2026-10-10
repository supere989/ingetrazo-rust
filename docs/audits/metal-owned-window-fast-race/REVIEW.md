# Own-window capture race correction

Actual native897 Intel acceptance completed positively in0.764s, before the old one-second window snapshot. The capture diagnostic then failed despite native presentation_passed=true. The complete real native artifacts are retained in metal897-native-result-review.

The diagnostic probe now atomically publishes its own NSWindow identity/counters at each event-loop observation. An optional capture acknowledgement holds only the own diagnostic window alive after the existing six positive presentation/exact resize/no pending ownership acceptance has already been reached. The unchanged15s bound is still checked first. No acknowledgement can replace positive presentation or terminal resource completion. Capture refusals/timeouts acknowledge in finally, so they never terminate or stall the independent presentation test prematurely.

Engineering controls use a real fast child with a delayed mocked capture CLI and a bounded acknowledgement: accepted acknowledgement/pixel capture, missing acknowledgement refusal, all3 prior capture-failure independence controls, five PNG negatives. This proves harness behavior, not native Cocoa presentation. Fresh actual Mac replay is mandatory.
