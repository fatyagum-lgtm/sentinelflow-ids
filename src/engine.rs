use crate::event::{Direction, Event, Protocol};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::net::IpAddr;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub window_ms: u64,
    pub retention_windows: u64,
    pub max_hosts: usize,
    pub max_pairs_per_host: usize,
    pub max_recent_ids: usize,
    pub max_alerts: usize,
    pub scan_ports: usize,
    pub scan_hosts: usize,
    pub rate_min: u64,
    pub rate_ratio: f64,
    pub transfer_bytes: u64,
    pub beacon_intervals: usize,
    pub beacon_cv: f64,
    pub beacon_min_interval_ms: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            window_ms: 60_000,
            retention_windows: 2,
            max_hosts: 256,
            max_pairs_per_host: 32,
            max_recent_ids: 32768,
            max_alerts: 256,
            scan_ports: 12,
            scan_hosts: 10,
            rate_min: 30,
            rate_ratio: 4.0,
            transfer_bytes: 5_000_000,
            beacon_intervals: 5,
            beacon_cv: 0.1,
            beacon_min_interval_ms: 1000,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if !(1000..=300000).contains(&self.window_ms)
            || !(1..=5).contains(&self.retention_windows)
            || !(1..=1024).contains(&self.max_hosts)
            || !(1..=64).contains(&self.max_pairs_per_host)
            || !(1..=65536).contains(&self.max_recent_ids)
            || !(1..=1000).contains(&self.max_alerts)
            || !(2..=1024).contains(&self.scan_ports)
            || !(2..=1024).contains(&self.scan_hosts)
            || !(2..=16).contains(&self.beacon_intervals)
            || self.rate_min < 2
            || self.rate_min > 1_000_000
            || !self.rate_ratio.is_finite()
            || !(1.1..=100.0).contains(&self.rate_ratio)
            || !self.beacon_cv.is_finite()
            || !(0.0..=1.0).contains(&self.beacon_cv)
            || !(1..=1_000_000_000_000).contains(&self.transfer_bytes)
            || !(1..=60000).contains(&self.beacon_min_interval_ms)
        {
            return Err("Configuration exceeds documented bounds".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Alert {
    pub id: String,
    pub timestamp_ms: u64,
    pub detector: String,
    pub name: String,
    pub source: IpAddr,
    pub destination: Option<IpAddr>,
    pub window_start_ms: u64,
    pub window_end_ms: u64,
    pub statistic: String,
    pub observed: f64,
    pub threshold: f64,
    pub baseline: Option<f64>,
    pub supporting_events: u64,
    pub severity: String,
    pub explanation: String,
    pub caveat: String,
}
#[derive(Default, Clone, Debug, Serialize)]
pub struct Metrics {
    pub received: u64,
    pub accepted: u64,
    pub invalid: u64,
    pub late: u64,
    pub duplicates: u64,
    pub capacity_dropped: u64,
    pub pair_observations_dropped: u64,
    pub evicted_hosts: u64,
    pub evicted_ids: u64,
    pub alerts_total: u64,
    pub alerts_evicted: u64,
    pub bytes: u64,
    pub peak_hosts: usize,
    pub peak_pairs: usize,
    pub peak_ids: usize,
    pub by_detector: BTreeMap<String, u64>,
}
#[derive(Default, Clone, Debug, Serialize)]
pub struct Bucket {
    pub start_ms: u64,
    pub events: u64,
    pub bytes: u64,
    pub alerts: u64,
}
#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub config: Config,
    pub metrics: Metrics,
    pub watermark_ms: Option<u64>,
    pub active_hosts: usize,
    pub active_pairs: usize,
    pub recent_ids: usize,
    pub alerts: Vec<Alert>,
    pub timeline: Vec<Bucket>,
}
#[derive(Default)]
struct Pair {
    last: Option<u64>,
    intervals: VecDeque<u64>,
    alerted: bool,
}
#[derive(Default)]
struct Host {
    window: u64,
    last_seen: u64,
    count: u64,
    previous: Option<u64>,
    bytes: u64,
    outbound_count: u64,
    ports: HashSet<u16>,
    targets: HashSet<IpAddr>,
    pairs: HashMap<(IpAddr, u16, Protocol), Pair>,
    fired: HashSet<&'static str>,
}
pub struct Engine {
    config: Config,
    metrics: Metrics,
    hosts: HashMap<IpAddr, Host>,
    ids: HashSet<String>,
    id_queue: VecDeque<(u64, String)>,
    watermark: Option<u64>,
    window: Option<u64>,
    alerts: VecDeque<Alert>,
    timeline: VecDeque<Bucket>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Accepted,
    Invalid,
    Late,
    Duplicate,
    Capacity,
}
impl Engine {
    pub fn new(config: Config) -> Result<Self, String> {
        config.validate()?;
        Ok(Self {
            config,
            metrics: Metrics::default(),
            hosts: HashMap::new(),
            ids: HashSet::new(),
            id_queue: VecDeque::new(),
            watermark: None,
            window: None,
            alerts: VecDeque::new(),
            timeline: VecDeque::new(),
        })
    }
    pub fn invalid_record(&mut self) {
        self.metrics.received = self.metrics.received.saturating_add(1);
        self.metrics.invalid = self.metrics.invalid.saturating_add(1);
    }
    fn advance(&mut self, window: u64) {
        if self.window == Some(window) {
            return;
        }
        let floor = window.saturating_sub(self.config.retention_windows * self.config.window_ms);
        let before = self.hosts.len();
        self.hosts.retain(|_, h| h.last_seen >= floor);
        self.metrics.evicted_hosts = self
            .metrics
            .evicted_hosts
            .saturating_add((before - self.hosts.len()) as u64);
        for host in self.hosts.values_mut() {
            let previous = if host.window + self.config.window_ms == window {
                Some(host.count)
            } else {
                None
            };
            let last_seen = host.last_seen;
            *host = Host {
                window,
                last_seen,
                previous,
                ..Host::default()
            };
        }
        while self.id_queue.front().is_some_and(|(ts, _)| *ts < floor) {
            let (_, id) = self.id_queue.pop_front().expect("front checked");
            self.ids.remove(&id);
            self.metrics.evicted_ids = self.metrics.evicted_ids.saturating_add(1);
        }
        self.window = Some(window);
        self.timeline.push_back(Bucket {
            start_ms: window,
            ..Bucket::default()
        });
        if self.timeline.len() > 120 {
            self.timeline.pop_front();
        }
    }
    pub fn process(&mut self, event: Event) -> Outcome {
        self.metrics.received = self.metrics.received.saturating_add(1);
        if event.validate().is_err() {
            self.metrics.invalid = self.metrics.invalid.saturating_add(1);
            return Outcome::Invalid;
        }
        if self.ids.contains(&event.id) {
            self.metrics.duplicates = self.metrics.duplicates.saturating_add(1);
            return Outcome::Duplicate;
        }
        if self.watermark.is_some_and(|last| event.timestamp_ms < last) {
            self.metrics.late = self.metrics.late.saturating_add(1);
            return Outcome::Late;
        }
        let window = event.timestamp_ms / self.config.window_ms * self.config.window_ms;
        self.advance(window);
        self.watermark = Some(event.timestamp_ms);
        if !self.hosts.contains_key(&event.source) && self.hosts.len() >= self.config.max_hosts {
            self.metrics.capacity_dropped = self.metrics.capacity_dropped.saturating_add(1);
            return Outcome::Capacity;
        }
        if self.ids.len() >= self.config.max_recent_ids {
            let (_, id) = self.id_queue.pop_front().expect("bounded id queue");
            self.ids.remove(&id);
            self.metrics.evicted_ids = self.metrics.evicted_ids.saturating_add(1);
        }
        self.ids.insert(event.id.clone());
        self.id_queue
            .push_back((event.timestamp_ms, event.id.clone()));
        self.metrics.accepted = self.metrics.accepted.saturating_add(1);
        self.metrics.bytes = self.metrics.bytes.saturating_add(event.bytes);
        let bucket = self.timeline.back_mut().expect("window initialized");
        bucket.events = bucket.events.saturating_add(1);
        bucket.bytes = bucket.bytes.saturating_add(event.bytes);
        let host = self.hosts.entry(event.source).or_insert_with(|| Host {
            window,
            ..Host::default()
        });
        host.last_seen = event.timestamp_ms;
        host.count = host.count.saturating_add(1);
        if host.ports.len() < self.config.scan_ports {
            host.ports.insert(event.destination_port);
        }
        if host.targets.len() < self.config.scan_hosts {
            host.targets.insert(event.destination);
        }
        if event.direction == Direction::Outbound {
            host.bytes = host.bytes.saturating_add(event.bytes);
            host.outbound_count = host.outbound_count.saturating_add(1);
        }
        let mut findings = Vec::new();
        if (host.ports.len() >= self.config.scan_ports
            || host.targets.len() >= self.config.scan_hosts)
            && host.fired.insert("scan")
        {
            let ports = host.ports.len() >= self.config.scan_ports;
            findings.push(alert(
                &event,
                &self.config,
                "scan",
                "Target diversity",
                "medium",
                if ports {
                    "distinct destination ports (capped lower bound)"
                } else {
                    "distinct destination hosts (capped lower bound)"
                },
                if ports {
                    host.ports.len() as f64
                } else {
                    host.targets.len() as f64
                },
                if ports {
                    self.config.scan_ports as f64
                } else {
                    self.config.scan_hosts as f64
                },
                None,
                host.count,
                "Many distinct targets within one event-time window.",
                "Administration, service discovery and authorized scanning can match.",
            ));
        }
        if let Some(previous) = host.previous.filter(|v| *v > 0) {
            let boundary =
                (previous as f64 * self.config.rate_ratio).max(self.config.rate_min as f64);
            if host.count as f64 >= boundary && host.fired.insert("rate") {
                findings.push(alert(&event,&self.config,"rate","Event-rate increase","high","current window event count",host.count as f64,boundary,Some(previous as f64),host.count,"Current count crossed the previous contiguous window count × ratio and minimum count.","Counts are flow records, not necessarily new connections. Workload bursts and partial reference windows can match."));
            }
        }
        if host.bytes >= self.config.transfer_bytes && host.fired.insert("transfer") {
            findings.push(alert(&event,&self.config,"transfer","Outbound volume","high","outbound bytes",host.bytes as f64,self.config.transfer_bytes as f64,None,host.outbound_count,"Outbound aggregate bytes crossed the configured volume threshold.","Backup, replication and downloads can match; this does not confirm exfiltration. Direction is supplied by the input."));
        }
        let key = (event.destination, event.destination_port, event.protocol);
        if host.pairs.contains_key(&key) || host.pairs.len() < self.config.max_pairs_per_host {
            let pair = host.pairs.entry(key).or_default();
            if let Some(last) = pair.last {
                pair.intervals.push_back(event.timestamp_ms - last);
                if pair.intervals.len() > 16 {
                    pair.intervals.pop_front();
                }
            }
            pair.last = Some(event.timestamp_ms);
            if !pair.alerted && pair.intervals.len() >= self.config.beacon_intervals {
                let mean = pair.intervals.iter().map(|x| *x as f64).sum::<f64>()
                    / pair.intervals.len() as f64;
                let variance = pair
                    .intervals
                    .iter()
                    .map(|x| (*x as f64 - mean).powi(2))
                    .sum::<f64>()
                    / pair.intervals.len() as f64;
                let cv = if mean > 0.0 {
                    variance.sqrt() / mean
                } else {
                    f64::INFINITY
                };
                if mean >= self.config.beacon_min_interval_ms as f64 && cv <= self.config.beacon_cv
                {
                    pair.alerted = true;
                    let mut finding = alert(
                        &event,
                        &self.config,
                        "beacon",
                        "Periodic communication",
                        "medium",
                        "interarrival coefficient of variation",
                        cv,
                        self.config.beacon_cv,
                        Some(mean),
                        pair.intervals.len() as u64 + 1,
                        "Low population CV across recent interarrival intervals; CV must be at or below threshold.",
                        "Scheduled jobs, health checks and polling can be periodic. This does not verify command-and-control.",
                    );
                    finding.destination = Some(event.destination);
                    finding.id.push_str(&format!(
                        ":{}:{}:{:?}",
                        event.destination, event.destination_port, event.protocol
                    ));
                    findings.push(finding);
                }
            }
        } else {
            self.metrics.pair_observations_dropped =
                self.metrics.pair_observations_dropped.saturating_add(1);
        }
        self.metrics.peak_hosts = self.metrics.peak_hosts.max(self.hosts.len());
        self.metrics.peak_pairs = self
            .metrics
            .peak_pairs
            .max(self.hosts.values().map(|h| h.pairs.len()).sum());
        self.metrics.peak_ids = self.metrics.peak_ids.max(self.ids.len());
        for finding in findings {
            self.metrics.alerts_total = self.metrics.alerts_total.saturating_add(1);
            let count = self
                .metrics
                .by_detector
                .entry(finding.detector.clone())
                .or_default();
            *count = count.saturating_add(1);
            let bucket = self.timeline.back_mut().expect("current bucket");
            bucket.alerts = bucket.alerts.saturating_add(1);
            if self.alerts.len() >= self.config.max_alerts {
                self.alerts.pop_front();
                self.metrics.alerts_evicted = self.metrics.alerts_evicted.saturating_add(1);
            }
            self.alerts.push_back(finding);
        }
        Outcome::Accepted
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            config: self.config.clone(),
            metrics: self.metrics.clone(),
            watermark_ms: self.watermark,
            active_hosts: self.hosts.len(),
            active_pairs: self.hosts.values().map(|h| h.pairs.len()).sum(),
            recent_ids: self.ids.len(),
            alerts: self.alerts.iter().cloned().collect(),
            timeline: self.timeline.iter().cloned().collect(),
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn alert(
    e: &Event,
    c: &Config,
    detector: &str,
    name: &str,
    severity: &str,
    statistic: &str,
    observed: f64,
    threshold: f64,
    baseline: Option<f64>,
    supporting_events: u64,
    explanation: &str,
    caveat: &str,
) -> Alert {
    let start = e.timestamp_ms / c.window_ms * c.window_ms;
    Alert {
        id: format!("{start}:{}:{detector}", e.source),
        timestamp_ms: e.timestamp_ms,
        detector: detector.into(),
        name: name.into(),
        source: e.source,
        destination: None,
        window_start_ms: start,
        window_end_ms: start + c.window_ms,
        statistic: statistic.into(),
        observed,
        threshold,
        baseline,
        supporting_events,
        severity: severity.into(),
        explanation: explanation.into(),
        caveat: caveat.into(),
    }
}
