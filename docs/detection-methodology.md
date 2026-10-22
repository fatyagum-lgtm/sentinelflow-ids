# Detection and synthetic quality protocol

| Rule | Default decision | Interpretation |
|---|---|---|
| Scan | >=12 distinct destination ports OR >=10 destinations/source/window | Legitimate discovery can trigger; repeated targets do not add diversity |
| Rate | count >= max(30, previous count ×4), requiring a positive contiguous reference | Partial initial windows and bursts can exaggerate changes |
| Transfer | outbound bytes >=5,000,000/source/window | Input direction is trusted; volume is not proof of exfiltration |
| Beacon | >=5 intervals, mean >=1000ms, population SD/mean <=0.1 | Scheduled benign services also qualify |

Scan/rate/transfer emit once per source/window; beacon once per pair/window.
Beacon keeps at most 16 intervals, resets at boundaries and rejects zero mean.
These are rules, not ML or calibrated risk probabilities.

Seven scenarios (background, scan, spike, transfer, beacon, scheduled, mixed)
run across seeds 7,19,42,73,101. Truth stays outside the Event schema/engine.
The unit is scenario × seed × source × detector. Any matching alert predicts
positive; repeated alerts collapse. Only expected source/detector pairs are
positive: 740 units, 40 positive and 700 negative.

Precision=TP/(TP+FP), recall=TP/(TP+FN), FPR=FP/(FP+TN); undefined ratios are null.
Delay is event time from annotated onset to the first matching post-onset alert,
reported only among detected patterns. Misses are excluded, not assigned zero.
All fixture expected patterns occur after their source's baseline events.

Sensitive/default/strict profiles change several parameters together, so they
are not single-variable causal experiments. The window study changes duration
to 30/60/120 seconds while retention stays two windows; elapsed retention changes.
Fixed attack placement interacts with boundaries. State peaks are counts, not bytes.

The static comparator counts windows/source with count >= rate_min; adaptive
counts rate-alert windows. Counts are not an equal-FPR calibrated contest.
Rules and fixtures were designed together. Separate truth does not remove design
leakage. Seeds vary background, not attack shape. There is no trained model,
independent holdout, operational prevalence estimate or adversarial guarantee.
