use sentinelflow_ids::{
    engine::{Config, Engine, Outcome},
    event::{Direction, Event, Protocol},
    generator::{self, BASE_MS},
    ingestion::JsonLines,
};
use std::{io::Cursor, net::Ipv4Addr};

fn flow(id: u64, offset: u64) -> Event {
    Event {
        id: format!("test-{id}"),
        timestamp_ms: BASE_MS + offset,
        source: Ipv4Addr::new(192, 0, 2, 1).into(),
        destination: Ipv4Addr::new(198, 51, 100, 1).into(),
        source_port: 40_001,
        destination_port: 443,
        protocol: Protocol::Tcp,
        packets: 1,
        bytes: 100,
        direction: Direction::Outbound,
        duration_ms: None,
        tcp_flags: None,
    }
}

fn alert_count(engine: &Engine, detector: &str) -> u64 {
    engine
        .snapshot()
        .metrics
        .by_detector
        .get(detector)
        .copied()
        .unwrap_or(0)
}

#[test]
fn schema_rejects_unknown_fields_wrong_types_and_bad_addresses() {
    let value = serde_json::to_value(flow(1, 0)).unwrap();
    for (key, replacement) in [
        ("source", serde_json::json!("not-an-ip")),
        ("packets", serde_json::json!(-1)),
        ("protocol", serde_json::json!("FTP")),
        ("truth", serde_json::json!("attack")),
    ] {
        let mut changed = value.clone();
        changed[key] = replacement;
        assert!(serde_json::from_value::<Event>(changed).is_err(), "{key}");
    }
}

#[test]
fn validation_enforces_numeric_and_protocol_bounds() {
    assert!(flow(1, 0).validate().is_ok());
    let mut event = flow(1, 0);
    event.bytes = 1_000_000_001;
    assert!(event.validate().is_err());
    event = flow(1, 0);
    event.timestamp_ms = u64::MAX;
    assert!(event.validate().is_err());
    event = flow(1, 0);
    event.id = "../private".into();
    assert!(event.validate().is_err());
    event = flow(1, 0);
    event.protocol = Protocol::Icmp;
    assert!(event.validate().is_err());
    event.source_port = 0;
    event.destination_port = 0;
    assert!(event.validate().is_ok());
    event.tcp_flags = Some(2);
    assert!(event.validate().is_err());
}

#[test]
fn invalid_config_is_rejected_before_engine_creation() {
    for config in [
        Config {
            window_ms: 0,
            ..Config::default()
        },
        Config {
            max_hosts: 0,
            ..Config::default()
        },
        Config {
            beacon_intervals: 17,
            ..Config::default()
        },
        Config {
            beacon_cv: f64::NAN,
            ..Config::default()
        },
        Config {
            rate_ratio: f64::INFINITY,
            ..Config::default()
        },
    ] {
        assert!(Engine::new(config).is_err());
    }
    assert!(serde_json::from_str::<Config>(r#"{"unknown": 1}"#).is_err());
}

#[test]
fn scan_counts_distinct_targets_and_fires_once_per_window() {
    let mut engine = Engine::new(Config {
        scan_ports: 3,
        ..Config::default()
    })
    .unwrap();
    for index in 0..20 {
        engine.process(flow(index, index));
    }
    assert_eq!(alert_count(&engine, "scan"), 0);
    for index in 20..30 {
        let mut event = flow(index, index);
        event.destination_port = 1_000 + index as u16;
        engine.process(event);
    }
    assert_eq!(alert_count(&engine, "scan"), 1);
    let snapshot = engine.snapshot();
    let alert = snapshot
        .alerts
        .iter()
        .find(|a| a.detector == "scan")
        .unwrap();
    assert_eq!(alert.observed, 3.0);
}

#[test]
fn host_diversity_also_triggers_scan() {
    let mut engine = Engine::new(Config {
        scan_hosts: 2,
        ..Config::default()
    })
    .unwrap();
    engine.process(flow(0, 0));
    let mut second = flow(1, 1);
    second.destination = Ipv4Addr::new(198, 51, 100, 2).into();
    engine.process(second);
    assert_eq!(alert_count(&engine, "scan"), 1);
}

#[test]
fn rate_requires_contiguous_nonzero_baseline_and_minimum() {
    let mut engine = Engine::new(Config {
        rate_min: 6,
        rate_ratio: 2.0,
        ..Config::default()
    })
    .unwrap();
    for index in 0..3 {
        engine.process(flow(index, index));
    }
    assert_eq!(alert_count(&engine, "rate"), 0);
    for index in 0..5 {
        engine.process(flow(10 + index, 60_000 + index));
    }
    assert_eq!(alert_count(&engine, "rate"), 0);
    engine.process(flow(20, 60_005));
    assert_eq!(alert_count(&engine, "rate"), 1);
    let alert = engine
        .snapshot()
        .alerts
        .into_iter()
        .find(|a| a.detector == "rate")
        .unwrap();
    assert_eq!(alert.baseline, Some(3.0));
    assert_eq!(alert.threshold, 6.0);
}

#[test]
fn missing_window_disables_rate_reference() {
    let mut engine = Engine::new(Config {
        rate_min: 2,
        rate_ratio: 2.0,
        ..Config::default()
    })
    .unwrap();
    engine.process(flow(0, 0));
    for index in 0..10 {
        engine.process(flow(index + 1, 120_000 + index));
    }
    assert_eq!(alert_count(&engine, "rate"), 0);
}

#[test]
fn transfer_counts_only_outbound_bytes_and_evidence() {
    let mut engine = Engine::new(Config {
        transfer_bytes: 200,
        ..Config::default()
    })
    .unwrap();
    let mut inbound = flow(0, 0);
    inbound.direction = Direction::Inbound;
    inbound.bytes = 1_000;
    engine.process(inbound);
    engine.process(flow(1, 1));
    assert_eq!(alert_count(&engine, "transfer"), 0);
    engine.process(flow(2, 2));
    assert_eq!(alert_count(&engine, "transfer"), 1);
    let alert = engine
        .snapshot()
        .alerts
        .into_iter()
        .find(|a| a.detector == "transfer")
        .unwrap();
    assert_eq!(alert.supporting_events, 2);
    assert_eq!(alert.observed, 200.0);
}

#[test]
fn beacon_requires_enough_intervals_and_low_cv() {
    let mut engine = Engine::new(Config::default()).unwrap();
    for index in 0..5 {
        engine.process(flow(index, index * 5_000));
    }
    assert_eq!(alert_count(&engine, "beacon"), 0);
    engine.process(flow(5, 25_000));
    assert_eq!(alert_count(&engine, "beacon"), 1);
    let alert = engine
        .snapshot()
        .alerts
        .into_iter()
        .find(|a| a.detector == "beacon")
        .unwrap();
    assert_eq!(alert.observed, 0.0);
    assert_eq!(alert.baseline, Some(5_000.0));
    assert_eq!(alert.supporting_events, 6);
    let mut irregular = Engine::new(Config::default()).unwrap();
    for (index, offset) in [0, 1_000, 3_000, 4_000, 24_000, 25_000]
        .into_iter()
        .enumerate()
    {
        irregular.process(flow(index as u64, offset));
    }
    assert_eq!(alert_count(&irregular, "beacon"), 0);
}

#[test]
fn equal_timestamps_do_not_create_nan_beacon_results() {
    let mut engine = Engine::new(Config::default()).unwrap();
    for index in 0..20 {
        assert_eq!(engine.process(flow(index, 0)), Outcome::Accepted);
    }
    assert_eq!(alert_count(&engine, "beacon"), 0);
    assert!(serde_json::to_string(&engine.snapshot()).is_ok());
}

#[test]
fn half_open_windows_reset_detector_state() {
    let mut engine = Engine::new(Config {
        transfer_bytes: 200,
        ..Config::default()
    })
    .unwrap();
    engine.process(flow(0, 59_999));
    engine.process(flow(1, 60_000));
    assert_eq!(alert_count(&engine, "transfer"), 0);
    engine.process(flow(2, 60_001));
    let snapshot = engine.snapshot();
    assert_eq!(alert_count(&engine, "transfer"), 1);
    assert_eq!(snapshot.timeline.len(), 2);
    assert_eq!(snapshot.alerts[0].window_start_ms, BASE_MS + 60_000);
    assert_eq!(snapshot.alerts[0].window_end_ms, BASE_MS + 120_000);
}

#[test]
fn duplicates_late_and_invalid_are_visible_without_counting_evidence() {
    let mut engine = Engine::new(Config::default()).unwrap();
    assert_eq!(engine.process(flow(1, 100)), Outcome::Accepted);
    assert_eq!(engine.process(flow(1, 100)), Outcome::Duplicate);
    assert_eq!(engine.process(flow(2, 99)), Outcome::Late);
    let mut invalid = flow(3, 101);
    invalid.packets = 0;
    assert_eq!(engine.process(invalid), Outcome::Invalid);
    engine.invalid_record();
    let metrics = engine.snapshot().metrics;
    assert_eq!(
        (
            metrics.received,
            metrics.accepted,
            metrics.duplicates,
            metrics.late,
            metrics.invalid
        ),
        (5, 1, 1, 1, 2)
    );
}

#[test]
fn host_retention_frees_capacity_without_unbounded_growth() {
    let mut engine = Engine::new(Config {
        max_hosts: 1,
        retention_windows: 1,
        ..Config::default()
    })
    .unwrap();
    engine.process(flow(0, 0));
    let mut other = flow(1, 1);
    other.source = Ipv4Addr::new(192, 0, 2, 2).into();
    assert_eq!(engine.process(other.clone()), Outcome::Capacity);
    other.timestamp_ms = BASE_MS + 120_000;
    assert_eq!(engine.process(other), Outcome::Accepted);
    let snapshot = engine.snapshot();
    assert_eq!(snapshot.active_hosts, 1);
    assert_eq!(snapshot.metrics.evicted_hosts, 1);
    assert_eq!(snapshot.metrics.capacity_dropped, 1);
}

#[test]
fn pair_id_alert_and_timeline_caps_are_enforced() {
    let mut engine = Engine::new(Config {
        max_pairs_per_host: 1,
        max_recent_ids: 2,
        max_alerts: 1,
        transfer_bytes: 100,
        ..Config::default()
    })
    .unwrap();
    for index in 0..130 {
        engine.process(flow(index * 2, index * 60_000));
        let mut second = flow(index * 2 + 1, index * 60_000 + 1);
        second.destination_port = 80;
        engine.process(second);
    }
    let snapshot = engine.snapshot();
    assert!(snapshot.active_pairs <= 1);
    assert!(snapshot.recent_ids <= 2);
    assert_eq!(snapshot.timeline.len(), 120);
    assert_eq!(snapshot.alerts.len(), 1);
    assert_eq!(snapshot.metrics.pair_observations_dropped, 130);
    assert_eq!(snapshot.metrics.alerts_evicted, 129);
    assert!(snapshot.metrics.evicted_ids > 0);
}

#[test]
fn empty_stream_has_no_alerts_or_watermark() {
    let snapshot = Engine::new(Config::default()).unwrap().snapshot();
    assert_eq!(snapshot.metrics.received, 0);
    assert!(snapshot.alerts.is_empty());
    assert!(snapshot.watermark_ms.is_none());
    assert!(
        JsonLines::new(Cursor::new(Vec::<u8>::new()), 4096)
            .next()
            .is_none()
    );
}

#[test]
fn malformed_and_oversized_lines_recover_and_final_newline_is_optional() {
    let valid = serde_json::to_string(&flow(0, 0)).unwrap();
    let text = format!("bad JSON\n{}\n{valid}", "x".repeat(10_000));
    let records: Vec<_> = JsonLines::new(Cursor::new(text), 4096).collect();
    assert_eq!(records.len(), 3);
    assert!(records[0].as_ref().unwrap_err().contains("Line 1"));
    assert!(records[1].as_ref().unwrap_err().contains("exceeds 4096"));
    assert!(records[2].is_ok());
}

#[test]
fn underlying_read_failure_is_an_error_not_an_empty_stream() {
    struct Broken;
    impl std::io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("synthetic error"))
        }
    }
    impl std::io::BufRead for Broken {
        fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
            Err(std::io::Error::other("synthetic error"))
        }
        fn consume(&mut self, _: usize) {}
    }
    let mut lines = JsonLines::new(Broken, 4096);
    assert!(lines.next().unwrap().is_err());
    assert!(lines.next().is_none());
}

#[test]
fn replay_is_deterministic_and_truth_is_separate() {
    let mut results = Vec::new();
    for _ in 0..2 {
        let fixture = generator::generate("mixed", 42).unwrap();
        assert!(
            fixture
                .events
                .windows(2)
                .all(|pair| pair[0].timestamp_ms <= pair[1].timestamp_ms)
        );
        let mut engine = Engine::new(Config::default()).unwrap();
        for event in fixture.events {
            assert!(event.validate().is_ok());
            let json = serde_json::to_value(&event).unwrap();
            assert!(json.get("scenario").is_none());
            assert!(json.get("expected").is_none());
            assert_eq!(engine.process(event), Outcome::Accepted);
        }
        let snapshot = engine.snapshot();
        for truth in fixture.truth.expected {
            assert!(
                snapshot
                    .alerts
                    .iter()
                    .any(|a| a.detector == truth.detector && a.source == truth.source)
            );
        }
        results.push(serde_json::to_string(&snapshot).unwrap());
    }
    assert_eq!(results[0], results[1]);
    assert!(generator::generate("unknown", 42).is_err());
}

#[test]
fn scheduled_benign_traffic_exposes_beacon_false_positive() {
    let fixture = generator::generate("scheduled", 42).unwrap();
    assert!(fixture.truth.expected.is_empty());
    let mut engine = Engine::new(Config::default()).unwrap();
    for event in fixture.events {
        engine.process(event);
    }
    assert!(
        engine
            .snapshot()
            .alerts
            .iter()
            .any(|a| a.detector == "beacon"
                && a.source == std::net::IpAddr::from(Ipv4Addr::new(192, 0, 2, 40)))
    );
}

#[test]
fn alerts_include_explanation_evidence_window_and_stable_id() {
    let mut engine = Engine::new(Config {
        transfer_bytes: 100,
        ..Config::default()
    })
    .unwrap();
    engine.process(flow(0, 0));
    let snapshot = engine.snapshot();
    let value = serde_json::to_value(&snapshot.alerts[0]).unwrap();
    for name in [
        "id",
        "timestamp_ms",
        "detector",
        "name",
        "source",
        "destination",
        "window_start_ms",
        "window_end_ms",
        "statistic",
        "observed",
        "threshold",
        "baseline",
        "supporting_events",
        "severity",
        "explanation",
        "caveat",
    ] {
        assert!(value.get(name).is_some(), "{name}");
    }
    assert_eq!(value["id"], format!("{BASE_MS}:192.0.2.1:transfer"));
}
