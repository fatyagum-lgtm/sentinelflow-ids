//! Reproducible synthetic evaluation; deliberately no generalization claim.
use sentinelflow_ids::{
    engine::{Config, Engine},
    generator,
    ingestion::JsonLines,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Write},
    process::Command,
    time::Instant,
};
fn ratio(n: u64, d: u64) -> Option<f64> {
    (d > 0).then(|| n as f64 / d as f64)
}
fn median(v: &mut [u64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_unstable();
    let n = v.len();
    Some(if n.is_multiple_of(2) {
        (v[n / 2 - 1] as f64 + v[n / 2] as f64) / 2.0
    } else {
        v[n / 2] as f64
    })
}
fn quality(label: &str, config: Config) -> Value {
    let (mut tp, mut fp, mut tn, mut missed) = (0u64, 0u64, 0u64, 0u64);
    let mut cases = Vec::new();
    let mut delays = Vec::new();
    let (mut peak_hosts, mut peak_pairs, mut peak_ids) = (0, 0, 0);
    let (mut static_windows, mut adaptive_windows) = (0, 0);
    let mut total_alerts = 0;
    for scenario in generator::SCENARIOS {
        for seed in [7, 19, 42, 73, 101] {
            let fixture = generator::generate(scenario, seed).expect("known scenario");
            let sources: BTreeSet<_> = fixture.events.iter().map(|e| e.source).collect();
            let mut counts = BTreeMap::new();
            let mut engine = Engine::new(config.clone()).expect("valid profile");
            for e in fixture.events {
                *counts
                    .entry((e.source, e.timestamp_ms / config.window_ms))
                    .or_insert(0u64) += 1;
                engine.process(e);
            }
            let snap = engine.snapshot();
            static_windows += counts.values().filter(|&&n| n >= config.rate_min).count();
            adaptive_windows += snap.alerts.iter().filter(|a| a.detector == "rate").count();
            total_alerts += snap.metrics.alerts_total;
            peak_hosts = peak_hosts.max(snap.metrics.peak_hosts);
            peak_pairs = peak_pairs.max(snap.metrics.peak_pairs);
            peak_ids = peak_ids.max(snap.metrics.peak_ids);
            let (mut ct, mut cf, mut cn, mut cm) = (0, 0, 0, 0);
            let mut cd = Vec::new();
            for source in sources {
                for detector in ["scan", "rate", "transfer", "beacon"] {
                    let truth = fixture
                        .truth
                        .expected
                        .iter()
                        .find(|e| e.source == source && e.detector == detector);
                    let found = snap
                        .alerts
                        .iter()
                        .any(|a| a.source == source && a.detector == detector);
                    match (truth.is_some(), found) {
                        (true, true) => {
                            ct += 1;
                            let onset = truth.expect("present").onset_ms;
                            if let Some(a) = snap.alerts.iter().find(|a| {
                                a.source == source
                                    && a.detector == detector
                                    && a.timestamp_ms >= onset
                            }) {
                                cd.push(a.timestamp_ms - onset)
                            }
                        }
                        (false, true) => cf += 1,
                        (true, false) => cm += 1,
                        (false, false) => cn += 1,
                    }
                }
            }
            tp += ct;
            fp += cf;
            tn += cn;
            missed += cm;
            delays.extend(&cd);
            cases.push(json!({"scenario":scenario,"seed":seed,"tp":ct,"fp":cf,"fn":cm,"tn":cn,"alerts":snap.metrics.alerts_total,"delays_ms":cd}));
        }
    }
    json!({"label":label,"config":config,"tp":tp,"fp":fp,"fn":missed,"tn":tn,"precision":ratio(tp,tp+fp),"recall":ratio(tp,tp+missed),"false_positive_rate":ratio(fp,fp+tn),"median_delay_ms":median(&mut delays),"total_alerts":total_alerts,"peak_hosts":peak_hosts,"peak_pairs":peak_pairs,"peak_ids":peak_ids,"static_rate_alert_windows":static_windows,"adaptive_rate_alert_windows":adaptive_windows,"cases":cases})
}
fn rss() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find(|l| l.starts_with("VmHWM:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}
fn trial(n: u64) -> Value {
    let mut warm = Engine::new(Config::default()).unwrap();
    for i in 0..5000 {
        warm.process(generator::workload(i, 7));
    }
    drop(warm);
    let mut engine = Engine::new(Config::default()).unwrap();
    let mut times = Vec::new();
    let start = Instant::now();
    for i in 0..n {
        let e = generator::workload(i, 42);
        if i % 100 == 0 {
            let t = Instant::now();
            engine.process(e);
            times.push(t.elapsed().as_nanos() as u64);
        } else {
            engine.process(e);
        }
    }
    let seconds = start.elapsed().as_secs_f64();
    times.sort_unstable();
    let m = engine.snapshot().metrics;
    json!({"events":n,"accepted":m.accepted,"seconds":seconds,"events_per_second":n as f64/seconds,"latency_p50_ns":times[times.len()/2],"latency_p95_ns":times[(times.len()*95/100).min(times.len()-1)],"latency_samples":times.len(),"rss_peak_kib":rss(),"peak_hosts":m.peak_hosts,"peak_pairs":m.peak_pairs,"peak_ids":m.peak_ids})
}
fn resilience() -> Vec<Value> {
    let mut result = Vec::new();
    let mut e = Engine::new(Config::default()).unwrap();
    let f = generator::workload(1, 42);
    e.process(f.clone());
    e.process(f);
    e.process(generator::workload(0, 42));
    let m = e.snapshot().metrics;
    result.push(json!({"name":"duplicate and late input","passed":m.duplicates==1&&m.late==1&&m.accepted==1,"metrics":m}));
    let valid = serde_json::to_string(&generator::workload(0, 42)).unwrap();
    let input = format!("bad\n{}\n{valid}", "x".repeat(10000));
    let mut e = Engine::new(Config::default()).unwrap();
    for r in JsonLines::new(Cursor::new(input), 4096) {
        match r {
            Ok(v) => {
                e.process(v);
            }
            Err(_) => e.invalid_record(),
        }
    }
    let m = e.snapshot().metrics;
    result.push(json!({"name":"malformed and oversized recovery","passed":m.invalid==2&&m.accepted==1,"metrics":m}));
    let e = Engine::new(Config::default()).unwrap();
    result.push(json!({"name":"empty input","passed":e.snapshot().metrics.received==0}));
    let mut e = Engine::new(Config {
        max_hosts: 4,
        max_recent_ids: 8,
        ..Config::default()
    })
    .unwrap();
    for i in 0..1000 {
        e.process(generator::workload(i, 42));
    }
    let s = e.snapshot();
    result.push(json!({"name":"host and ID caps","passed":s.active_hosts<=4&&s.recent_ids<=8&&s.metrics.capacity_dropped>0,"metrics":s.metrics}));
    let mut e = Engine::new(Config {
        max_pairs_per_host: 1,
        ..Config::default()
    })
    .unwrap();
    for i in 0..20 {
        let mut f = generator::workload(i, 42);
        f.source = "192.0.2.1".parse().unwrap();
        f.destination_port = 1000 + i as u16;
        e.process(f);
    }
    let s = e.snapshot();
    result.push(json!({"name":"pair cap","passed":s.active_pairs==1&&s.metrics.pair_observations_dropped==19,"metrics":s.metrics}));
    result
}
fn output(command: &str, args: &[&str]) -> String {
    Command::new(command)
        .args(args)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().into())
        .unwrap_or_else(|| "unavailable".into())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if cfg!(debug_assertions) {
        return Err("Research requires --release".into());
    }
    if args.get(1).is_some_and(|a| a == "--trial") {
        let n: u64 = args.get(2).ok_or("Missing count")?.parse()?;
        if ![10000, 100000, 500000].contains(&n) {
            return Err("Unsupported trial size".into());
        }
        println!("{}", trial(n));
        return Ok(());
    }
    let directory = args
        .get(1)
        .ok_or("Usage: research <new-output-directory>")?;
    std::fs::create_dir_all(directory)?;
    let path = std::path::Path::new(directory).join("research.json");
    let mut file = std::fs::File::create_new(path)?;
    let mut benchmarks = Vec::new();
    for n in [10000, 100000, 500000] {
        let mut trials = Vec::new();
        for _ in 0..3 {
            let out = Command::new(std::env::current_exe()?)
                .args(["--trial", &n.to_string()])
                .output()?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string().into());
            }
            trials.push(serde_json::from_slice::<Value>(&out.stdout)?);
        }
        let mut rates: Vec<_> = trials
            .iter()
            .map(|v| v["events_per_second"].as_f64().unwrap())
            .collect();
        rates.sort_by(f64::total_cmp);
        benchmarks.push(json!({"events":n,"median_events_per_second":rates[1],"trials":trials}));
    }
    let thresholds = vec![
        quality(
            "sensitive",
            Config {
                scan_ports: 6,
                scan_hosts: 5,
                rate_min: 15,
                rate_ratio: 2.0,
                transfer_bytes: 2_000_000,
                beacon_intervals: 3,
                beacon_cv: 0.3,
                ..Config::default()
            },
        ),
        quality("default", Config::default()),
        quality(
            "strict",
            Config {
                scan_ports: 40,
                scan_hosts: 20,
                rate_min: 100,
                rate_ratio: 8.0,
                transfer_bytes: 12_000_000,
                beacon_cv: 0.01,
                beacon_intervals: 8,
                ..Config::default()
            },
        ),
    ];
    let windows: Vec<_> = [30000, 60000, 120000]
        .into_iter()
        .map(|w| {
            quality(
                &format!("{} seconds", w / 1000),
                Config {
                    window_ms: w,
                    ..Config::default()
                },
            )
        })
        .collect();
    let resilience = resilience();
    let queue = tokio::runtime::Runtime::new()?.block_on(sentinelflow_ids::api::queue_probe());
    if resilience.iter().any(|v| v["passed"] != true) || queue["passed"] != true {
        return Err("Resilience assertion failed".into());
    }
    let cpu = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .map(|l| l.split(':').nth(1).unwrap_or("unknown").trim().to_string())
        })
        .unwrap_or_else(|| "unavailable".into());
    let report = json!({"schema_version":1,"generated_unix_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis(),"environment":{"cpu":cpu,"logical_cpus":std::thread::available_parallelism()?.get(),"os":output("uname",&["-srmo"]),"rustc":output("rustc",&["--version"]),"build_profile":"release","host_condition":"shared container; no CPU isolation or cache control"},"protocol":{"scope":"Synthetic-scenario evaluation, not real-world intrusion-detection accuracy.","unit":"scenario × seed × source × detector; any alert predicts positive; repeats collapse. Seven scenarios, five seeds. No held-out corpus.","throughput":"Generator + engine, 3 fresh processes per size, 5000 warmup excluded; no parsing/disk/API/capture.","latency":"Every 100th process call; empirical p50/p95 including timer overhead and deterministic sampling bias.","memory":"Linux VmHWM KiB per process including warmup/runtime/allocator/samples; null when unavailable.","delay":"Event-time median among detected patterns only; misses excluded.","thresholds":"Bundled profiles, not single-variable causal experiment.","windows":"Window duration varies; retention remains two windows. State peaks are counts, not bytes."},"quality":thresholds[1],"thresholds":thresholds,"windows":windows,"benchmarks":benchmarks,"resilience":resilience,"queue":queue});
    serde_json::to_writer_pretty(&mut file, &report)?;
    writeln!(file)?;
    file.flush()?;
    println!("Wrote measured research report to {directory}/research.json");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("Research error: {e}");
        std::process::exit(2)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn undefined_ratios_and_even_median() {
        assert_eq!(ratio(0, 0), None);
        assert_eq!(median(&mut [4, 1, 3, 2]), Some(2.5));
    }
    #[test]
    fn scheduled_false_positives_are_preserved() {
        let q = quality("test", Config::default());
        assert_eq!(
            (
                q["tp"].as_u64(),
                q["fp"].as_u64(),
                q["fn"].as_u64(),
                q["tn"].as_u64()
            ),
            (Some(40), Some(5), Some(0), Some(695))
        );
    }
    #[test]
    fn resilience_checks_pass() {
        assert!(resilience().iter().all(|v| v["passed"] == true));
    }
}
