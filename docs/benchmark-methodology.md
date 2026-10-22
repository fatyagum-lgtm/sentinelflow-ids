# Benchmark and resilience methods

Run `cargo run --release --locked --bin research -- <new-directory>`. Debug
research runs and existing output reports are rejected. Compiler/CPU/OS/visible
CPU count are recorded. No Python is required for the project or experiments.

Each size (10k/100k/500k) runs three fresh child trials. A 5000-event warmup is
excluded, then a new engine is created. Throughput times generator + engine,
not JSON parsing, disk, HTTP, UI or capture. The workload cycles 128 sources and
four service ports, with deterministic bytes and reserved addresses. It is
unlabeled stress traffic. All times/counts and median throughput are retained.

Every 100th process call is timed with Instant, excluding generation. Empirical
p50/p95 include clock overhead and deterministic sampling bias, not end-to-end
latency. Short runs are sensitive to scheduling; no CPU pinning, cache control,
confidence intervals or isolated-host claim is made.

Linux VmHWM reports peak process RSS in KiB (null elsewhere). It includes warmup,
runtime, allocator and sample vectors, not just engine state. Host/pair/ID peaks
are counts. Window ablations report retention counts, not separate RSS effects.

Resilience checks cover duplicate/late input, malformed/oversized recovery, empty
streams, host/ID caps and pair caps. A real 64-item Tokio channel probe slows the
consumer by a requested 50µs per event (actual timer resolution differs), then
checks accepted count, waits and capacity. It demonstrates backpressure, not
production overload handling or packet-loss performance. API integration tests
also check stop, conflict, body/rate limits, host/header and invalid requests.
