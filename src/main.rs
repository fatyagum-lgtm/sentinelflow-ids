use sentinelflow_ids::{
    engine::{Config, Engine},
    generator,
    ingestion::JsonLines,
};
use std::{
    env,
    fs::{self, File},
    io::{self, BufReader, BufWriter, Read, Write},
    path::Path,
    process::ExitCode,
    thread,
    time::Duration,
};

const HELP: &str = "SentinelFlow IDS — streaming anomaly laboratory

USAGE
  sentinelflow-ids generate <scenario> <seed> <new-directory>
  sentinelflow-ids replay <events.jsonl> [options]
  sentinelflow-ids --help

SCENARIOS
  background | scan | spike | transfer | beacon | scheduled | mixed

REPLAY OPTIONS
  --config <file.json>   Partial Config JSON; unknown fields rejected
  --max-events <n>       Record limit, including malformed lines (default 1000000)
  --line-bytes <n>       Maximum record bytes, including newline (256..65536; default 4096)
  --speed <factor>       0 = fastest; 0.1..10000 = timed replay (default 0)
  --output <new.json>    Save snapshot without overwriting an existing file

Replay prints a JSON snapshot to stdout and a human summary to stderr.
The ordered-input policy drops older timestamps and recently cached IDs.
Timed replay caps each inter-event wait at 10 seconds; no network traffic is sent.
Exit codes: 0 = processed without drops; 2 = usage/I/O/configuration error;
3 = report produced with rejected records, exhausted record limit, or state loss.
Server: serve [port] (default 8787). Workload: workload <count> <seed> <new.jsonl>. Research: cargo run --release --bin research -- <new-directory>.";

struct ReplayOptions {
    config: Config,
    max_events: u64,
    line_bytes: usize,
    speed: f64,
    output: Option<String>,
}

fn parse_options(args: &[String]) -> Result<ReplayOptions, String> {
    let mut options = ReplayOptions {
        config: Config::default(),
        max_events: 1_000_000,
        line_bytes: 4096,
        speed: 0.0,
        output: None,
    };
    let mut seen = std::collections::HashSet::new();
    let (pairs, remainder) = args.as_chunks::<2>();
    for pair in pairs {
        let flag = pair[0].as_str();
        let value = &pair[1];
        if !seen.insert(flag) {
            return Err(format!("Repeated option: {flag}"));
        }
        match flag {
            "--config" => {
                let mut data = Vec::new();
                File::open(value)
                    .map_err(|e| format!("Cannot open configuration: {e}"))?
                    .take(16_385)
                    .read_to_end(&mut data)
                    .map_err(|e| format!("Cannot read configuration: {e}"))?;
                if data.len() > 16_384 {
                    return Err("Configuration exceeds 16384 bytes".into());
                }
                options.config = serde_json::from_slice(&data)
                    .map_err(|e| format!("Invalid configuration: {e}"))?;
                options.config.validate()?;
            }
            "--max-events" => {
                options.max_events = value.parse().map_err(|_| "Invalid record limit")?
            }
            "--line-bytes" => {
                options.line_bytes = value.parse().map_err(|_| "Invalid line limit")?
            }
            "--speed" => options.speed = value.parse().map_err(|_| "Invalid replay speed")?,
            "--output" => options.output = Some(value.clone()),
            _ => return Err(format!("Unknown option: {flag}")),
        }
    }
    if !remainder.is_empty() {
        return Err("Every replay option requires a value".into());
    }
    if !(1..=10_000_000).contains(&options.max_events) {
        return Err("Record limit must be 1..10000000".into());
    }
    if !(256..=65_536).contains(&options.line_bytes) {
        return Err("Line limit must be 256..65536".into());
    }
    if !options.speed.is_finite()
        || (options.speed != 0.0 && !(0.1..=10_000.0).contains(&options.speed))
    {
        return Err("Replay speed must be 0 or 0.1..10000".into());
    }
    Ok(options)
}

fn write_json(writer: impl Write, value: &impl serde::Serialize) -> Result<(), String> {
    let mut writer = writer;
    serde_json::to_writer_pretty(&mut writer, value)
        .map_err(|e| format!("Cannot write JSON: {e}"))?;
    writeln!(writer).map_err(|e| format!("Cannot finish JSON: {e}"))?;
    writer
        .flush()
        .map_err(|e| format!("Cannot flush JSON: {e}"))
}

fn generate(args: &[String]) -> Result<u8, String> {
    if args.len() != 3 {
        return Err("generate requires scenario, seed and a new directory".into());
    }
    let seed = args[1]
        .parse()
        .map_err(|_| "Seed must be an unsigned 64-bit integer")?;
    let fixture = generator::generate(&args[0], seed)?;
    let directory = Path::new(&args[2]);
    fs::create_dir(directory).map_err(|e| format!("Cannot create new output directory: {e}"))?;
    let file = File::create_new(directory.join("events.jsonl"))
        .map_err(|e| format!("Cannot create events: {e}"))?;
    let mut writer = BufWriter::new(file);
    for event in &fixture.events {
        serde_json::to_writer(&mut writer, event)
            .map_err(|e| format!("Cannot write event: {e}"))?;
        writeln!(writer).map_err(|e| format!("Cannot write newline: {e}"))?;
    }
    writer
        .flush()
        .map_err(|e| format!("Cannot flush events: {e}"))?;
    write_json(
        BufWriter::new(
            File::create_new(directory.join("truth.json"))
                .map_err(|e| format!("Cannot create truth: {e}"))?,
        ),
        &fixture.truth,
    )?;
    eprintln!(
        "Generated {} synthetic events; truth saved separately.",
        fixture.events.len()
    );
    Ok(0)
}

fn replay(args: &[String]) -> Result<u8, String> {
    let input = args.first().ok_or("replay requires an input JSONL file")?;
    let options = parse_options(&args[1..])?;
    let file = File::open(input).map_err(|e| format!("Cannot open input: {e}"))?;
    if !file
        .metadata()
        .map_err(|e| format!("Cannot inspect input: {e}"))?
        .is_file()
    {
        return Err("Input must be a regular file".into());
    }
    let mut records = JsonLines::new(BufReader::new(file), options.line_bytes);
    let mut engine = Engine::new(options.config)?;
    let mut previous: Option<u64> = None;
    let mut count = 0;
    for record in records.by_ref().take(options.max_events as usize) {
        count += 1;
        match record {
            Ok(event) => {
                if options.speed > 0.0
                    && let Some(last) = previous
                {
                    let seconds =
                        event.timestamp_ms.saturating_sub(last) as f64 / 1000.0 / options.speed;
                    thread::sleep(Duration::from_secs_f64(seconds.min(10.0)));
                }
                previous = Some(previous.unwrap_or(0).max(event.timestamp_ms));
                engine.process(event);
            }
            Err(reason) => {
                engine.invalid_record();
                if count <= 20 {
                    eprintln!("{reason}");
                }
            }
        }
    }
    // One bounded look-ahead distinguishes exactly-at-limit EOF from truncation.
    let truncated = count == options.max_events && records.next().is_some();
    if truncated {
        eprintln!("Record limit reached: remaining input was not analyzed.");
    }
    let snapshot = engine.snapshot();
    let report = serde_json::json!({ "schema_version": 1, "input_truncated": truncated, "snapshot": &snapshot });
    if let Some(path) = options.output {
        write_json(
            BufWriter::new(
                File::create_new(path).map_err(|e| format!("Cannot create new report: {e}"))?,
            ),
            &report,
        )?;
    }
    write_json(io::stdout().lock(), &report)?;
    let metrics = &snapshot.metrics;
    eprintln!(
        "Received {}, accepted {}, invalid {}, late {}, duplicate {}, capacity-dropped {}; alerts {} (retained {}).",
        metrics.received,
        metrics.accepted,
        metrics.invalid,
        metrics.late,
        metrics.duplicates,
        metrics.capacity_dropped,
        metrics.alerts_total,
        snapshot.alerts.len()
    );
    for alert in &snapshot.alerts {
        eprintln!(
            "{} {}: {} = {:.3}, threshold {:.3}; {}",
            alert.detector,
            alert.source,
            alert.statistic,
            alert.observed,
            alert.threshold,
            alert.caveat
        );
    }
    Ok(
        if truncated
            || metrics.received != metrics.accepted
            || metrics.pair_observations_dropped > 0
            || metrics.alerts_evicted > 0
        {
            3
        } else {
            0
        },
    )
}

fn run(args: &[String]) -> Result<u8, String> {
    match args.first().map(String::as_str) {
        Some("--help" | "-h") if args.len() == 1 => {
            println!("{HELP}");
            Ok(0)
        }
        Some("generate") => generate(&args[1..]),
        Some("replay") => replay(&args[1..]),
        Some("serve") if args.len() <= 2 => {
            let port = args
                .get(1)
                .map_or(Ok(8787u16), |s| s.parse())
                .map_err(|_| "Invalid port")?;
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?
                .block_on(sentinelflow_ids::api::serve(port))?;
            Ok(0)
        }
        Some("workload") if args.len() == 4 => {
            let n: u64 = args[1].parse().map_err(|_| "Invalid count")?;
            if !(1..=10_000_000).contains(&n) {
                return Err("Count must be 1..10000000".into());
            }
            let seed = args[2].parse().map_err(|_| "Invalid seed")?;
            let mut out = BufWriter::new(File::create_new(&args[3]).map_err(|e| e.to_string())?);
            for i in 0..n {
                serde_json::to_writer(&mut out, &generator::workload(i, seed))
                    .map_err(|e| e.to_string())?;
                writeln!(out).map_err(|e| e.to_string())?;
            }
            out.flush().map_err(|e| e.to_string())?;
            Ok(0)
        }
        _ => Err("Unknown or missing command. Use --help.".into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => ExitCode::from(code),
        Err(reason) => {
            eprintln!("Error: {reason}");
            ExitCode::from(2)
        }
    }
}
