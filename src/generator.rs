//! Small deterministic fixtures. Truth never enters the detector's Event schema.
use crate::event::{Direction, Event, Protocol};
use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr};

pub const BASE_MS: u64 = 1_767_225_600_000 / 60_000 * 60_000;
pub const SCENARIOS: &[&str] = &[
    "background",
    "scan",
    "spike",
    "transfer",
    "beacon",
    "scheduled",
    "mixed",
];

#[derive(Debug, Clone, Serialize)]
pub struct Truth {
    pub scenario: String,
    pub seed: u64,
    pub expected: Vec<Expected>,
    pub note: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct Expected {
    pub detector: &'static str,
    pub source: IpAddr,
    pub onset_ms: u64,
}

pub struct Fixture {
    pub events: Vec<Event>,
    pub truth: Truth,
}

fn source(host: u8) -> IpAddr {
    Ipv4Addr::new(192, 0, 2, host).into()
}

fn event(sequence: usize, offset: u64, host: u8, port: u16, bytes: u64) -> Event {
    Event {
        id: format!("flow-{sequence:06}"),
        timestamp_ms: BASE_MS + offset,
        source: source(host),
        destination: Ipv4Addr::new(198, 51, 100, 1).into(),
        source_port: 40_000 + u16::from(host),
        destination_port: port,
        protocol: Protocol::Tcp,
        packets: 10,
        bytes,
        direction: Direction::Outbound,
        duration_ms: Some(20),
        tcp_flags: None,
    }
}

fn next_random(state: &mut u64) -> u64 {
    // Explicit wrapping arithmetic gives the same sequence on every target.
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1);
    *state
}

pub fn generate(scenario: &str, seed: u64) -> Result<Fixture, String> {
    if !SCENARIOS.contains(&scenario) {
        return Err(format!("Unknown scenario; choose {}", SCENARIOS.join(", ")));
    }
    let mut events = Vec::with_capacity(900);
    let mut random = seed;
    // Four independent sources; jitter and a few service ports form background.
    for second in 0..180_u64 {
        for host in 101..=104 {
            let jitter = next_random(&mut random) % 900;
            let port = [80, 443, 8080][(next_random(&mut random) % 3) as usize];
            let bytes = 500 + next_random(&mut random) % 5_000;
            events.push(event(
                events.len(),
                second * 1_000 + jitter,
                host,
                port,
                bytes,
            ));
        }
    }
    let mut expected = Vec::new();
    if scenario == "scan" || scenario == "mixed" {
        for index in 0..30_u16 {
            events.push(event(
                events.len(),
                121_000 + u64::from(index) * 100,
                10,
                1_000 + index,
                800,
            ));
        }
        expected.push(Expected {
            detector: "scan",
            source: source(10),
            onset_ms: BASE_MS + 121_000,
        });
    }
    if scenario == "spike" || scenario == "mixed" {
        for start in [0, 60_000] {
            for index in 0..5 {
                events.push(event(
                    events.len(),
                    start + 1_000 + index * 300,
                    20,
                    443,
                    800,
                ));
            }
        }
        for index in 0..50 {
            events.push(event(events.len(), 121_000 + index * 300, 20, 443, 800));
        }
        expected.push(Expected {
            detector: "rate",
            source: source(20),
            onset_ms: BASE_MS + 121_000,
        });
    }
    if scenario == "transfer" || scenario == "mixed" {
        for offset in [126_000, 131_000, 139_000, 147_000] {
            events.push(event(events.len(), offset, 30, 443, 2_000_000));
        }
        expected.push(Expected {
            detector: "transfer",
            source: source(30),
            onset_ms: BASE_MS + 126_000,
        });
    }
    if matches!(scenario, "beacon" | "scheduled" | "mixed") {
        for index in 0..8 {
            events.push(event(events.len(), 121_000 + index * 5_000, 40, 443, 800));
        }
        if scenario != "scheduled" {
            expected.push(Expected {
                detector: "beacon",
                source: source(40),
                onset_ms: BASE_MS + 121_000,
            });
        }
    }
    events.sort_by(|a, b| (a.timestamp_ms, &a.id).cmp(&(b.timestamp_ms, &b.id)));
    Ok(Fixture {
        events,
        truth: Truth {
            scenario: scenario.into(),
            seed,
            expected,
            note: "Synthetic scenario labels, not verified intrusions. Scheduled polling intentionally resembles beaconing. Background may trigger false positives.",
        },
    })
}

/// Lazy, deterministic, unlabeled stress workload; no network traffic is sent.
pub fn workload(index: u64, seed: u64) -> Event {
    let mut state = seed.wrapping_add(index);
    let random = next_random(&mut state);
    let mut flow = event(
        0,
        index.saturating_mul(10).min(86_400_000_000),
        (index % 128 + 1) as u8,
        [80, 443, 8080, 8443][(index % 4) as usize],
        500 + random % 60_000,
    );
    flow.id = format!("bench-{seed}-{index}");
    flow.destination = Ipv4Addr::new(198, 51, 100, (index % 16 + 1) as u8).into();
    flow
}
