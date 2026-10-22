# Measured synthetic results

Generated 2026-10-18T04:32:52.991Z by the release research executable.
Raw cases, configurations and repeated trials: [research.json](../reports/research.json).

## Quality and threshold profiles

Known synthetic patterns, not independent real-world accuracy estimates.

| Profile | TP | FP | FN | TN | Precision | Recall | FPR | Detected median delay (ms) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| sensitive | 40 | 127 | 0 | 573 | 23.95% | 100.00% | 18.14% | 2350 |
| default | 40 | 5 | 0 | 695 | 88.89% | 100.00% | 0.71% | 10850 |
| strict | 0 | 0 | 40 | 700 | undefined | 0.00% | 0.00% | undefined |

Default false positives are benign scheduled-service cases. The strict profile
misses all expected patterns; zero false positives alone is not success. Sensitive
settings reduce detected-only delay but increase false positives. Bundled settings
change together, so this is not a one-parameter causal experiment.

## Window effects

| Window | TP | FP | FN | TN | Detected median delay (ms) | Peak hosts / pairs / IDs |
|---|---:|---:|---:|---:|---:|---|
| 30 seconds | 30 | 12 | 10 | 688 | 13000 | 8 / 45 / 455 |
| 60 seconds | 40 | 5 | 0 | 695 | 10850 | 8 / 45 / 822 |
| 120 seconds | 40 | 5 | 0 | 695 | 12350 | 8 / 45 / 822 |

Default static rate-alert windows: 440; adaptive: 10.
These are counts, not equal-FPR calibrated accuracy comparisons. Short windows
can eliminate a positive preceding-window reference. Retention seconds also
change; state counts do not measure window-specific resident memory.

## Throughput and memory

Three fresh child trials per size, 5,000-event warmup excluded. Generator + engine
only, excluding parsing, disk and HTTP. RSS covers the entire process.

| Events | Median events/s | Trial peak RSS (KiB) | Trial sampled p95 (ns) |
|---|---:|---|---|
| 10000 | 810,264 | 3660, 3708, 3644 | 1208, 1210, 1208 |
| 100000 | 785,761 | 5188, 5188, 5240 | 818, 1098, 743 |
| 500000 | 573,925 | 7064, 7176, 7060 | 1164, 1114, 1114 |

CPU: INTEL(R) XEON(R) PLATINUM 8573C; 8 visible logical CPUs.
OS: Linux 6.18.55 x86_64 GNU/Linux. Compiler: rustc 1.99.1 (58cf8f9c3 2026-10-13).
Condition: shared container; no CPU isolation or cache control. Short microbenchmarks do not establish
sustainable production throughput.

## Resilience and queue

- duplicate and late input: passed.
- malformed and oversized recovery: passed.
- empty input: passed.
- host and ID caps: passed.
- pair cap: passed.

Queue probe: 822/822 accepted, capacity 64,
observed peak 64, producer waits 740,
elapsed 1122.01ms; passed = true. Requested consumer
delays are subject to Tokio timer resolution.

See [methods](benchmark-methodology.md) and [limitations](limitations.md).
Recreate these tables: `node scripts/summarize.mjs`.
