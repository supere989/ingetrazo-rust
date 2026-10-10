# Independent Linux font resolver replay

Fresh original observer19requests match native paths and ascent/descent. Actual warm Rc reuse/counters, NUL refusal and refresh-retained owner controls pass; formats tests pass.

Code review found an expired Weak request can be inserted again without removing its old FIFO key, allowing queue growth under repeated font-cache retirement. Correction and actual regression requested from geometry worker. This finding prevents promoting the resolver as a fully bounded resource path. Mac/Windows providers and fallback segmentation remain required scope.
