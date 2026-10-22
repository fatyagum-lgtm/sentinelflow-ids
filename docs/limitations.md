# Limitations

A bounded offline/synthetic laboratory, not a deployed security product. No live
capture, PCAP parsing, prevention, authentication, persistence, multi-sensor
correlation, encrypted-payload inspection or trained ML is implemented.

Strict ordering drops late timestamps; forged future events advance the watermark.
Window boundaries split patterns; missing/partial references affect rates. ID
protection ends at TTL/FIFO eviction. Host/pair/set caps and alert eviction lose
information with visible counters. Benign scans/backups/scheduled services alert;
slow/distributed scans, jitter, baseline poisoning and small transfers may evade.

Known synthetic patterns and five background seeds do not establish real-world
accuracy or adversarial robustness. There is no held-out corpus or uncertainty
interval. Bundled profiles change multiple settings; retention seconds change
with windows. Throughput excludes parsing/I/O and RSS includes non-engine memory.

State starts empty per process; new replay replaces it, stop cannot resume.
Research loads only at startup. Browser seeds are JS safe integers, CLI seeds
are u64. Timed CLI waits cap at 10 seconds, compressing long gaps. Failed writes
may leave partial outputs. Only Linux and Chromium were locally exercised;
provided CI is not evidence of a hosted CI pass. No external security audit.
