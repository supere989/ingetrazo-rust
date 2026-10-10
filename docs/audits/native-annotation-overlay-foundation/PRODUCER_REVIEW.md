# Producer review follow-up

Immutable plan identity is checked immediately after constant density/DPI admission, before query/operation scans. Retained geometry admission now checked-sums vertices, indices, source-span metadata, ordered ranges, palette batches and gate arrays. This counts array payloads, not allocator overhead/capacity or font-native tables.

Glyph cache admits a bounded cold mask before planning FIFO eviction of exact cache-only owners. Live Arc owners cannot be evicted; their refusal preserves the ready mesh and existing masks. Failed candidates do not evict. Successful repeated density/phase churn evicts unused masks within count/byte limits; unchanged current mesh never re-rasterizes after eviction because immutable mesh reuse precedes glyph work. One cold provider mask and a bounded eviction-key scratch list are peak preparation work, separate from retained mask payload accounting.

Five focused producer controls PASS including retained-metadata budget, bounded churn and live-owner refusal. Query-depth and gate regression controls also pass in the retained focused log. No source-font mask/image or native application parity is inferred.
