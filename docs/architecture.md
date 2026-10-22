# Architecture and state bounds

The CLI consumes one bounded JSONL record at a time. The API accepts only a named
synthetic scenario, seed, speed and validated config, not arbitrary paths or targets.
Its producer builds at most 822 fixture events, then sends through a 64-item Tokio
channel. A consumer owns engine updates under a short mutex. Sends await capacity;
observed occupancy and waits are exposed. Stop signals both tasks, drops queued
work and preserves the partial snapshot. The fixture vector is resident too;
the large-workload generator is lazy and does not build a fixture vector.

Processing: validate → recent-ID check → late check → window/TTL advancement →
watermark → source admission → aggregation/detection. Equal timestamps are allowed.
The watermark advances even if a new source exceeds the host cap. IDs are bounded
by TTL and FIFO; an evicted ID can be accepted again. Counters expose losses.

Defaults: 60-second tumbling windows, two-window retention, 256 sources, 32 pairs
per source, 32768 IDs, 256 alerts and 120 observed timeline buckets. Pair keys are
destination IP/port/protocol within a source. Distinct sets and interval histories
are capped. TTL uses event time; no background wall-clock expiry exists. Expiry
removes timestamps below the inclusive boundary. Aggregates reset each window.
The rate reference is the preceding contiguous window only; gaps remove it.

GET routes: `/api/health`, `/api/status`, `/api/metrics`, `/api/alerts`,
`/api/timeline`, `/api/reports`, `/api/research`. POST `/api/replay` and `/api/stop`
require `X-SentinelFlow: 1`. Concurrent replay returns 409. JSON cap: 16 KiB.
Shared API request budget: 120/second. Research loads from a fixed file capped at
8 MiB on startup; missing data returns 404. Browser polling is serialized and
shows connection errors rather than inventing fresh measurements.
