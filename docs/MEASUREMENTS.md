# Measurements

Reference for the numbers the documentation quotes. Each row names the host and the date or catalogue it was measured on. Numbers marked "estimate" were not measured.

## Search latency (development host)

32-thread machine, 216k items.

| Quantity | Value |
|---|---|
| Search time inside `api` | 2–10 ms |
| Search time per request | 5–40 ms |
| Throughput, 32 concurrent clients | ~550 requests/s |

## Memory

### `api` on Linux containers

| Quantity | Value | Notes |
|---|---|---|
| Steady resident memory | ~0.4–1 GB | The memory-mapped Tantivy index is counted in page cache, not here |
| Peak while reloading a catalogue | ~1.9 GB (`VmHWM`) | The old and the new catalogue are held at once; `mem_limit: 3g` |
| Tag centroids (related searches) | 44.5 MB (`45,596 KB`) for 11,301 tags in production (2026-11-25, 224,965 items) | One copy is shared across reloads and rebuilt every 6 hours (30 MB for 7,396 tags on the development catalogue) |
| Matches of the tags that translate queries | up to about 20 MB per catalogue | Cache weight limit |

### Production, by container

Measured on 2026-11-20: 222,883 items on 4 GB RAM with 2 GB swap (921 MB of swap used, no active paging), `api` up 31 hours through 62 catalogue reloads. Anonymous memory is the process's own; page cache is file data the kernel reclaims first under the limit. The related-search centroids and the cache of translating-tag matches (see above) were added after this measurement.

| Container | Limit | Measured |
|---|---|---|
| `api` | `mem_limit: 3g` | 949 MB anonymous and 180 MB page cache; at most 2,542 MB of both together, reloads included (2026-11-20). Re-measured on 2026-11-25 with tag centroids enabled across 3 six-hour rebuilds (224,965 items): `VmRSS` 896 MB, `VmHWM` 1,899 MB, `memory.current` 1,911.2 MB, `memory.peak` 3,021.9 MB (page cache reclaimed cleanly; 0 MB swap) |
| `postgres` | none | 1,192 MB, almost all page cache (6 MB anonymous) |
| `imgproxy` | `mem_limit: 1g` | About 0.76 GB, almost all cold and swapped out (729 MB in swap, 31 MB resident) |
| `web` | none | 149 MB resident (92 MB anonymous, 57 MB page cache) and 121 MB in swap |
| `worker` | `mem_limit: 256m` | At most 29 MB over two days, the daily sweep included |

## Semantic search and embeddings

| Quantity | Value | Conditions |
|---|---|---|
| Embedding throughput | ~70 items/s | Development |
| First embedding of ~200k items | ~1 hour, a few dollars | `Qwen3-Embedding-4B` at $0.02/M tokens |
| One provider call for an unseen query | median 0.7 s, p95 1.2 s | Development |
| Disk added by vectors | ~0.5 GB in PostgreSQL plus ~0.5 GB for the HNSW index | |
| Tag centroid build | 3.8–3.9 s for 11,297–11,301 tags | Production (2026-11-25, 224,776–224,965 items); 1.0 s for 33,739 vectors on the development host |

## Start-up and crawling

| Quantity | Value | Notes |
|---|---|---|
| `api` start (migrations, then loading the catalogue) | ~20 s | |
| `textindex` rebuild at `api` start | ~10 s | Needed when the volume is lost, after an update that changes the index format, or when the catalogue's text changed since the index was built |
| Full crawl without a data bundle | 2–3 days for ~200k items | ~3,400 listing pages, one item request per unknown item at ~0.7 s pacing |
| Restoring the data bundle | ~5 min | |

## Image proxy

| Quantity | Value | Conditions |
|---|---|---|
| Images requested while scrolling to page 3 | 542 in 10 s, up to 341 in one second | 1440x900, no image cached |
| Per-client allowance for upstream requests | 100 a second, bursts of 2,000 | Configured limit, not a measurement; see [Service behaviour](SERVICES.md#image-proxy) |
