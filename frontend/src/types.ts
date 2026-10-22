export type Alert = {
  id: string;
  name: string;
  detector: string;
  source: string;
  destination: string | null;
  timestamp_ms: number;
  window_start_ms: number;
  window_end_ms: number;
  statistic: string;
  observed: number;
  threshold: number;
  baseline: number | null;
  supporting_events: number;
  severity: string;
  explanation: string;
  caveat: string;
};
export type Bucket = {
  start_ms: number;
  events: number;
  bytes: number;
  alerts: number;
};
export type Metrics = {
  received: number;
  accepted: number;
  bytes: number;
  alerts_total: number;
  late: number;
  duplicates: number;
  invalid: number;
  capacity_dropped: number;
  pair_observations_dropped: number;
  alerts_evicted: number;
  by_detector: Record<string, number>;
};
export type View = {
  phase: string;
  scenario: string;
  seed: number;
  total: number;
  elapsed_ms: number;
  events_per_second: number;
  queue_capacity: number;
  queue_peak_observed: number;
  producer_backpressure_waits: number;
  snapshot: {
    config: Record<string, number>;
    metrics: Metrics;
    active_hosts: number;
    active_pairs: number;
    alerts: Alert[];
    timeline: Bucket[];
  };
};
export type Quality = {
  label: string;
  tp: number;
  fp: number;
  fn: number;
  tn: number;
  precision: number | null;
  recall: number | null;
  false_positive_rate: number | null;
  median_delay_ms: number | null;
  peak_ids: number;
  peak_hosts: number;
  peak_pairs: number;
  total_alerts: number;
};
export type Research = {
  quality: Quality;
  thresholds: Quality[];
  windows: Quality[];
  benchmarks: {
    events: number;
    median_events_per_second: number;
    trials: { latency_p95_ns: number; rss_peak_kib: number | null }[];
  }[];
  environment: { cpu: string; rustc: string; host_condition: string };
  protocol: { unit: string };
  resilience: { name: string; passed: boolean }[];
  queue: { passed: boolean };
};
