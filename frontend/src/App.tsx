import { useEffect, useState } from "react";
import { api } from "./api";
import type { Alert, Bucket, View, Quality, Research } from "./types";
const names: Record<string, string> = {
  background: "Background",
  scan: "Target discovery",
  spike: "Rate increase",
  transfer: "Large transfer",
  beacon: "Periodic traffic",
  scheduled: "Benign scheduled service",
  mixed: "Mixed behavior",
};
const detectors = [
  [
    "scan",
    "Target diversity",
    "Distinct ports or destinations within a source window.",
    "Authorized discovery can match.",
  ],
  [
    "rate",
    "Event-rate increase",
    "Current count crosses a positive preceding-window baseline × ratio and a minimum.",
    "Bursts and partial reference windows can match.",
  ],
  [
    "transfer",
    "Outbound volume",
    "Aggregated outbound bytes cross a configured threshold.",
    "Volume does not confirm exfiltration.",
  ],
  [
    "beacon",
    "Periodic communication",
    "Population interval coefficient of variation falls below the configured limit.",
    "Scheduled benign services also look periodic.",
  ],
];
const number = (v: number) =>
  v.toLocaleString("en-US", { maximumFractionDigits: 1 });
const utc = (v: number) => new Date(v).toISOString().slice(11, 19);
const pct = (v: number | null) =>
  v === null ? "n/a" : `${(v * 100).toFixed(1)}%`;
export function Timeline({ buckets }: { buckets: Bucket[] }) {
  const [mode, setMode] = useState<"events" | "bytes">("events");
  const [selected, setSelected] = useState<number | null>(null);
  const max = Math.max(1, ...buckets.map((b) => b[mode]));
  const active = buckets.find((b) => b.start_ms === selected) ?? buckets.at(-1);
  return (
    <section className="panel">
      <div className="panel-head">
        <div>
          <small>EVENT-TIME OBSERVATIONS</small>
          <h2>Traffic through the window</h2>
        </div>
        <div className="segmented">
          {(["events", "bytes"] as const).map((m) => (
            <button
              key={m}
              aria-pressed={m === mode}
              onClick={() => setMode(m)}
            >
              {m}
            </button>
          ))}
        </div>
      </div>
      {buckets.length ? (
        <>
          <div className="plot">
            {buckets.map((b) => (
              <button
                className="bar-column"
                key={b.start_ms}
                aria-label={`${utc(b.start_ms)} UTC: ${number(b[mode])} ${mode}`}
                onClick={() => setSelected(b.start_ms)}
              >
                <span
                  className="bar"
                  style={{ height: `${Math.max(1, (b[mode] / max) * 100)}%` }}
                />
                <span className="tick">{utc(b.start_ms)}</span>
              </button>
            ))}
          </div>
          <p className="muted">
            UTC · {number(active!.events)} events /{" "}
            {number(active!.bytes / 1e6)} MB / {active!.alerts} alerts in
            selected window
          </p>
        </>
      ) : (
        <div className="empty">No observations yet. Start a replay.</div>
      )}
      <div className="panel-foot">
        Observed windows only <span>UTC · ordered input</span>
      </div>
    </section>
  );
}
export function Alerts({ alerts }: { alerts: Alert[] }) {
  const [filter, setFilter] = useState("all");
  const [selected, setSelected] = useState<string | null>(null);
  const visible = [...alerts]
    .reverse()
    .filter((a) => filter === "all" || a.detector === filter);
  const a = alerts.find((a) => a.id === selected);
  return (
    <section className="panel">
      <div className="panel-head">
        <div>
          <small>EXPLAINABLE FINDINGS</small>
          <h2>
            Alert stream <em>{alerts.length}</em>
          </h2>
        </div>
        <select
          aria-label="Filter alerts"
          value={filter}
          onChange={(e) => {
            setFilter(e.target.value);
            setSelected(null);
          }}
        >
          <option value="all">All detectors</option>
          {detectors.map((d) => (
            <option key={d[0]}>{d[0]}</option>
          ))}
        </select>
      </div>
      <div className="alert-head">
        <span>DETECTION / SOURCE</span>
        <span className="stat">EVIDENCE</span>
        <span>PRIORITY</span>
        <span>UTC</span>
      </div>
      {visible.length ? (
        visible.map((a) => (
          <button
            key={a.id}
            className="alert-row"
            onClick={() => setSelected(a.id)}
            aria-expanded={selected === a.id}
          >
            <span>
              <i className={a.detector} />
              {a.name}
              <small>
                {a.source}
                {a.destination ? ` → ${a.destination}` : ""}
              </small>
            </span>
            <span className="stat">
              {number(a.observed)}
              <small>{a.statistic}</small>
            </span>
            <span>
              <b className={`badge ${a.severity}`}>{a.severity}</b>
            </span>
            <time>{utc(a.timestamp_ms)}</time>
          </button>
        ))
      ) : (
        <p className="empty">
          No matching alerts. Absence of an alert does not establish safety.
        </p>
      )}
      {a && (
        <aside className="evidence" aria-label="Alert evidence">
          <button onClick={() => setSelected(null)}>Close ×</button>
          <h3>{a.name}</h3>
          <p>{a.explanation}</p>
          <dl>
            <dt>Observed / threshold</dt>
            <dd>
              {number(a.observed)} / {number(a.threshold)}
            </dd>
            <dt>Baseline / supporting events</dt>
            <dd>
              {a.baseline === null ? "none" : number(a.baseline)} /{" "}
              {a.supporting_events}
            </dd>
            <dt>Window (UTC)</dt>
            <dd>
              {new Date(a.window_start_ms).toISOString()} —{" "}
              {new Date(a.window_end_ms).toISOString()}
            </dd>
          </dl>
          <p className="warning">{a.caveat}</p>
        </aside>
      )}
    </section>
  );
}
function QualityTable({ values, title }: { values: Quality[]; title: string }) {
  return (
    <section className="panel">
      <h2>{title}</h2>
      <div className="table-scroll">
        <table>
          <thead>
            <tr>
              <th>Configuration</th>
              <th>Precision</th>
              <th>Recall</th>
              <th>FP / FN</th>
              <th>Detected delay</th>
              <th>Peak IDs</th>
            </tr>
          </thead>
          <tbody>
            {values.map((v) => (
              <tr key={v.label}>
                <td>{v.label}</td>
                <td>{pct(v.precision)}</td>
                <td>{pct(v.recall)}</td>
                <td>
                  {v.fp} / {v.fn}
                </td>
                <td>
                  {v.median_delay_ms === null
                    ? "n/a"
                    : `${(v.median_delay_ms / 1000).toFixed(2)}s`}
                </td>
                <td>{v.peak_ids}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="muted">
        Delay excludes misses. State counts are not resident memory. Profiles
        vary bundled settings.
      </p>
    </section>
  );
}
function ResearchPage() {
  const [data, setData] = useState<Research | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    let alive = true;
    api<Research>("/api/research")
      .then((v) => {
        if (alive) setData(v);
      })
      .catch((e) => {
        if (alive) setError(String(e));
      });
    return () => {
      alive = false;
    };
  }, []);
  if (error)
    return (
      <p role="alert" className="warning">
        {error}
      </p>
    );
  if (!data) return <p role="status">Loading measured research…</p>;
  const q = data.quality;
  const max = Math.max(
    ...data.benchmarks.map((b) => b.median_events_per_second),
  );
  return (
    <>
      <div className="cards">
        <Card
          label="SYNTHETIC PRECISION"
          value={pct(q.precision)}
          note="Source / detector presence units"
        />
        <Card
          label="SYNTHETIC RECALL"
          value={pct(q.recall)}
          note={`${q.tp} TP / ${q.fn} FN`}
        />
        <Card
          label="FALSE POSITIVES"
          value={String(q.fp)}
          note="Benign patterns can match"
        />
        <Card
          label="BENCHMARK TRIALS"
          value="9"
          note="Three fresh processes per size"
        />
      </div>
      <div className="columns">
        <section className="panel">
          <small>RELEASE ENGINE / REAL EXECUTION</small>
          <h2>Measured throughput</h2>
          {data.benchmarks.map((b) => (
            <div className="meter-row" key={b.events}>
              <span>{number(b.events)}</span>
              <div>
                <span
                  style={{
                    width: `${(b.median_events_per_second / max) * 100}%`,
                  }}
                />
              </div>
              <strong>
                {(b.median_events_per_second / 1e6).toFixed(2)}M/s
              </strong>
            </div>
          ))}
          <p className="muted">
            Median of three trials. Generator + engine only; no parsing, disk,
            API or capture.
          </p>
        </section>
        <section className="panel">
          <small>CONTROLLED SCENARIOS</small>
          <h2>Confusion counts</h2>
          <div className="confusion">
            {[
              ["True positive", q.tp],
              ["False positive", q.fp],
              ["False negative", q.fn],
              ["True negative", q.tn],
            ].map(([s, v]) => (
              <div key={s}>
                <small>{s}</small>
                <strong>{v}</strong>
              </div>
            ))}
          </div>
        </section>
      </div>
      <QualityTable title="The cost of a threshold" values={data.thresholds} />
      <QualityTable title="Time changes the evidence" values={data.windows} />
      <section className="panel">
        <h2>Performance conditions</h2>
        <div className="table-scroll">
          <table>
            <thead>
              <tr>
                <th>Records</th>
                <th>Median events/s</th>
                <th>Trial sampled p95 (µs)</th>
                <th>Trial peak RSS (KiB)</th>
              </tr>
            </thead>
            <tbody>
              {data.benchmarks.map((b) => (
                <tr key={b.events}>
                  <td>{number(b.events)}</td>
                  <td>{number(b.median_events_per_second)}</td>
                  <td>
                    {b.trials
                      .map((t) => (t.latency_p95_ns / 1000).toFixed(3))
                      .join(" / ")}
                  </td>
                  <td>
                    {b.trials.map((t) => t.rss_peak_kib ?? "n/a").join(" / ")}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <p>
          {data.environment.cpu} · {data.environment.rustc}
        </p>
        <p className="muted">
          {data.environment.host_condition}. Every 100th process call sampled;
          timer overhead applies. RSS includes warmup, runtime and allocator.
        </p>
      </section>
      <section className="panel">
        <small>REPRODUCIBLE, NOT UNIVERSAL</small>
        <h2>What these results mean</h2>
        <p>{data.protocol.unit}</p>
        <p>
          Synthetic-scenario evaluation, not real-world intrusion-detection
          accuracy. Seeds vary background; fixed attack patterns are known.
          Scheduled legitimate services produce beacon false positives.
        </p>
        <div className="checks">
          {data.resilience.map((c) => (
            <span key={c.name}>
              {c.passed ? "✓" : "✗"} {c.name}
            </span>
          ))}
          <span>
            {data.queue.passed ? "✓" : "✗"} Bounded slow-consumer queue
          </span>
        </div>
        <a href="/api/research" download="research.json">
          Download measured research JSON ↗
        </a>
      </section>
    </>
  );
}
function Card({
  label,
  value,
  note,
}: {
  label: string;
  value: string;
  note: string;
}) {
  return (
    <div className="card">
      <small>{label}</small>
      <strong
        data-testid={
          label === "EVENTS ACCEPTED" ? "accepted-events" : undefined
        }
      >
        {value}
      </strong>
      <p>{note}</p>
    </div>
  );
}
export default function App() {
  const [page, setPage] = useState("Overview");
  const [live, setLive] = useState<View | null>(null);
  const [connection, setConnection] = useState("");
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  const [scenario, setScenario] = useState("mixed");
  const [seed, setSeed] = useState("42");
  const [speed, setSpeed] = useState("60");
  const [windowMs, setWindowMs] = useState("60000");
  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const v = await api<View>("/api/status");
        if (active) {
          setLive(v);
          setConnection("");
        }
      } catch (e) {
        if (active) setConnection(String(e));
      } finally {
        if (active) timer = setTimeout(poll, 500);
      }
    };
    void poll();
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, []);
  const running = live?.phase === "running";
  async function action() {
    setError("");
    if (
      !running &&
      (!Number.isSafeInteger(Number(seed)) || Number(seed) < 0 || !seed.trim())
    ) {
      setError("Seed must be a nonnegative safe integer.");
      return;
    }
    setPending(true);
    try {
      await api(
        running ? "/api/stop" : "/api/replay",
        running
          ? {}
          : {
              scenario,
              seed: Number(seed),
              speed: Number(speed),
              config: { window_ms: Number(windowMs) },
            },
      );
      setLive(await api<View>("/api/status"));
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
    }
  }
  const m = live?.snapshot.metrics;
  const titles: Record<string, string> = {
    Overview: "Observe the signal.",
    Replay: "Follow the stream.",
    "Detection lab": "Understand every alert.",
    Research: "Measure the tradeoffs.",
  };
  return (
    <div className="app">
      <aside className="sidebar">
        <a className="brand" href="#">
          <b>SF</b>
          <span>
            SentinelFlow<small>STREAMING IDS</small>
          </span>
        </a>
        <small className="nav-title">RESEARCH WORKSPACE / 01</small>
        <nav aria-label="Main navigation">
          {["Overview", "Replay", "Detection lab", "Research"].map((p, i) => (
            <button
              key={p}
              className={page === p ? "active" : ""}
              onClick={() => setPage(p)}
            >
              <span>{["◫", "▷", "⌘", "▥"][i]}</span>
              {p}
              <small>0{i + 1}</small>
            </button>
          ))}
        </nav>
        <div className="sidebar-note">
          <div>▂ ▅ ▃ ▇ ▃ ▅ ▂</div>
          <p>Behavior, made visible.</p>
          <small>
            A bounded Rust engine.
            <br />
            Interpretable observations.
            <br />
            Reproducible experiments.
          </small>
        </div>
        <small className="sidebar-foot">
          ● LOCAL LABORATORY
          <br />
          Synthetic traffic · no live capture
        </small>
      </aside>
      <main>
        <header>
          <span>WORKSPACE / {page}</span>
          <span className="connection">
            {connection
              ? "CONNECTION ERROR"
              : live
                ? "LOCAL API"
                : "CONNECTING"}{" "}
            <small>RUST + AXUM / v0.1</small>
          </span>
        </header>
        <div className="content">
          <div className="hero">
            <div>
              <small>
                STREAMING INTRUSION DETECTION & BEHAVIORAL ANALYTICS
              </small>
              <h1>{titles[page]}</h1>
              <p>
                A local laboratory for network anomalies — from event to
                evidence.
              </p>
            </div>
            <div className="engine-status">
              <small>ENGINE STATUS</small>
              <strong>● {live?.phase.toUpperCase() ?? "CONNECTING"}</strong>
              <span>{live ? names[live.scenario] : "Awaiting API"}</span>
            </div>
          </div>
          <p className="notice">
            <b>LAB NOTE</b> Synthetic observations, not a production security
            verdict. An alert is a pattern to investigate.
          </p>
          {connection && (
            <p role="alert" className="warning">
              API connection failed; displayed data may be stale. {connection}
            </p>
          )}
          {error && (
            <p role="alert" className="warning">
              {error}
            </p>
          )}
          {page !== "Research" && (
            <section className="controls">
              <div>
                <small>SYNTHETIC REPLAY</small>
                <h2>Choose an observation.</h2>
              </div>
              <label>
                Scenario
                <select
                  value={scenario}
                  disabled={running || pending}
                  onChange={(e) => setScenario(e.target.value)}
                >
                  {Object.entries(names).map(([k, v]) => (
                    <option key={k} value={k}>
                      {v}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Seed
                <input
                  aria-label="Random seed"
                  value={seed}
                  disabled={running || pending}
                  onChange={(e) => setSeed(e.target.value)}
                />
              </label>
              <label>
                Window
                <select
                  value={windowMs}
                  disabled={running || pending}
                  onChange={(e) => setWindowMs(e.target.value)}
                >
                  {[30000, 60000, 120000].map((v) => (
                    <option key={v} value={v}>
                      {v / 1000} seconds
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Replay speed
                <select
                  value={speed}
                  disabled={running || pending}
                  onChange={(e) => setSpeed(e.target.value)}
                >
                  {[20, 60, 200, 0].map((v) => (
                    <option key={v} value={v}>
                      {v === 0 ? "Fastest" : `${v}×`}
                    </option>
                  ))}
                </select>
              </label>
              <button
                className="primary"
                disabled={pending || !live || !!connection}
                onClick={() => void action()}
              >
                {pending
                  ? "Working…"
                  : running
                    ? "Stop replay"
                    : "Start replay"}{" "}
                ↗
              </button>
            </section>
          )}
          {page === "Research" ? (
            <ResearchPage />
          ) : page === "Detection lab" ? (
            <>
              <div className="detector-grid">
                {detectors.map(([id, title, desc, caveat]) => (
                  <section className="panel" key={id}>
                    <small>{id.toUpperCase()} / BEHAVIORAL RULE</small>
                    <h2>{title}</h2>
                    <p>{desc}</p>
                    <p className="warning">{caveat}</p>
                  </section>
                ))}
              </div>
              <section className="panel">
                <h2>Active configuration</h2>
                <p>
                  Changes apply to the next replay. Thresholds are validated in
                  Rust; full configuration is available through the CLI or API.
                </p>
                <div className="config-grid">
                  {Object.entries(live?.snapshot.config ?? {}).map(([k, v]) => (
                    <div key={k}>
                      <small>{k}</small>
                      <strong>{v}</strong>
                    </div>
                  ))}
                </div>
              </section>
              <Alerts alerts={live?.snapshot.alerts ?? []} />
            </>
          ) : (
            <>
              <div className="cards">
                <Card
                  label="EVENTS ACCEPTED"
                  value={number(m?.accepted ?? 0)}
                  note={`${number(m?.received ?? 0)} records received`}
                />
                <Card
                  label="BEHAVIORAL ALERTS"
                  value={number(m?.alerts_total ?? 0)}
                  note={`${live?.snapshot.alerts.length ?? 0} retained for inspection`}
                />
                <Card
                  label="OBSERVED VOLUME"
                  value={`${number((m?.bytes ?? 0) / 1e6)} MB`}
                  note="Aggregate input flow bytes"
                />
                <Card
                  label="ACTIVE SOURCES"
                  value={`${live?.snapshot.active_hosts ?? 0} / ${live?.snapshot.config.max_hosts ?? 256}`}
                  note={`${live?.snapshot.active_pairs ?? 0} tracked destination pairs`}
                />
              </div>
              {page === "Replay" && (
                <section className="panel">
                  <small>BOUNDED REPLAY PIPELINE</small>
                  <h2>
                    {number(m?.accepted ?? 0)} / {number(live?.total ?? 0)}{" "}
                    events
                  </h2>
                  <progress
                    max={Math.max(1, live?.total ?? 0)}
                    value={m?.accepted ?? 0}
                  />
                  <p>
                    {number(live?.events_per_second ?? 0)} events/s average
                    including playback waits ·{" "}
                    {number((live?.elapsed_ms ?? 0) / 1000)}s elapsed
                  </p>
                  <p className="muted">
                    Queue capacity {live?.queue_capacity ?? 64} · observed peak{" "}
                    {live?.queue_peak_observed ?? 0} · producer backpressure
                    waits {live?.producer_backpressure_waits ?? 0}. Stop
                    preserves a partial snapshot; no resume.
                  </p>
                </section>
              )}
              <div className="columns">
                <Timeline buckets={live?.snapshot.timeline ?? []} />
                <section className="panel">
                  <small>FOUR SIGNALS / ONE STREAM</small>
                  <h2>Detector activity</h2>
                  {detectors.map(([id]) => (
                    <div className="meter-row" key={id}>
                      <span>{id}</span>
                      <div>
                        <span
                          style={{
                            width: `${((m?.by_detector[id] ?? 0) / Math.max(1, ...Object.values(m?.by_detector ?? {}))) * 100}%`,
                          }}
                        />
                      </div>
                      <strong>{m?.by_detector[id] ?? 0}</strong>
                    </div>
                  ))}
                  <p className="muted">
                    Heuristic priorities. No confidence percentages.
                  </p>
                </section>
              </div>
              <Alerts alerts={live?.snapshot.alerts ?? []} />
              <div className="accounting">
                <span>RECORD ACCOUNTING</span>
                {[
                  ["late", m?.late],
                  ["duplicate", m?.duplicates],
                  ["invalid", m?.invalid],
                  ["host-cap drops", m?.capacity_dropped],
                  ["pair-cap drops", m?.pair_observations_dropped],
                  ["alerts evicted", m?.alerts_evicted],
                ].map(([s, n]) => (
                  <span key={s}>
                    {n ?? 0} {s}
                  </span>
                ))}
                <a href="/api/reports">Export JSON ↗</a>
              </div>
            </>
          )}
          <footer>
            <span>SENTINELFLOW IDS / Independent research implementation</span>
            <span>Local by design. Explicit about uncertainty.</span>
          </footer>
        </div>
      </main>
    </div>
  );
}
