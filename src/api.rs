//! Loopback-only synthetic replay API. No arbitrary paths or network targets.
use crate::{
    engine::{Config, Engine, Snapshot},
    generator,
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Request, State},
    http::{HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::Read,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, watch};
use tower_http::services::ServeDir;
const CAPACITY: usize = 64;
#[derive(Clone)]
pub struct AppState {
    live: Arc<Mutex<Live>>,
    research: Option<Value>,
    budget: Arc<Mutex<(Instant, u32)>>,
}
struct Live {
    engine: Engine,
    phase: &'static str,
    total: usize,
    scenario: String,
    seed: u64,
    start: Option<Instant>,
    elapsed: f64,
    peak: usize,
    waits: u64,
    cancel: Option<watch::Sender<bool>>,
}
#[derive(Serialize)]
pub struct View {
    pub phase: &'static str,
    pub total: usize,
    pub scenario: String,
    pub seed: u64,
    pub elapsed_ms: f64,
    pub events_per_second: f64,
    pub queue_capacity: usize,
    pub queue_peak_observed: usize,
    pub producer_backpressure_waits: u64,
    pub snapshot: Snapshot,
}
impl Default for AppState {
    fn default() -> Self {
        Self::new(None)
    }
}
impl AppState {
    pub fn new(research: Option<Value>) -> Self {
        Self {
            live: Arc::new(Mutex::new(Live {
                engine: Engine::new(Config::default()).expect("default valid"),
                phase: "idle",
                total: 0,
                scenario: "mixed".into(),
                seed: 42,
                start: None,
                elapsed: 0.0,
                peak: 0,
                waits: 0,
                cancel: None,
            })),
            research,
            budget: Arc::new(Mutex::new((Instant::now(), 0))),
        }
    }
    pub fn view(&self) -> View {
        let l = self.live.lock().expect("state lock");
        let seconds = if l.phase == "running" {
            l.start.map_or(0.0, |t| t.elapsed().as_secs_f64())
        } else {
            l.elapsed
        };
        let snapshot = l.engine.snapshot();
        View {
            phase: l.phase,
            total: l.total,
            scenario: l.scenario.clone(),
            seed: l.seed,
            elapsed_ms: seconds * 1000.0,
            events_per_second: if seconds > 0.0 {
                snapshot.metrics.accepted as f64 / seconds
            } else {
                0.0
            },
            queue_capacity: CAPACITY,
            queue_peak_observed: l.peak,
            producer_backpressure_waits: l.waits,
            snapshot,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Replay {
    scenario: String,
    seed: u64,
    speed: u32,
    #[serde(default)]
    config: Config,
}
type ApiResult = Result<Json<Value>, (StatusCode, String)>;
async fn replay(State(state): State<AppState>, Json(input): Json<Replay>) -> ApiResult {
    if input.speed > 10000 {
        return Err((StatusCode::BAD_REQUEST, "Speed must be 0..10000".into()));
    }
    let engine = Engine::new(input.config).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let fixture = generator::generate(&input.scenario, input.seed)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let (cancel, mut cancelled) = watch::channel(false);
    {
        let mut l = state.live.lock().expect("state lock");
        if l.phase == "running" {
            return Err((StatusCode::CONFLICT, "Replay already running".into()));
        }
        *l = Live {
            engine,
            phase: "running",
            total: fixture.events.len(),
            scenario: input.scenario,
            seed: input.seed,
            start: Some(Instant::now()),
            elapsed: 0.0,
            peak: 0,
            waits: 0,
            cancel: Some(cancel),
        };
    }
    let (sender, mut receiver) = mpsc::channel(CAPACITY);
    let producer_state = state.clone();
    let mut producer_cancel = cancelled.clone();
    let producer = tokio::spawn(async move {
        let mut previous = None;
        for event in fixture.events {
            if *producer_cancel.borrow() {
                break;
            }
            if input.speed > 0
                && let Some(last) = previous
            {
                let delay = Duration::from_secs_f64(
                    event.timestamp_ms.saturating_sub(last) as f64
                        / 1000.0
                        / f64::from(input.speed),
                );
                tokio::select! {_ = tokio::time::sleep(delay)=>{},_ = producer_cancel.changed()=>{break}}
            }
            previous = Some(event.timestamp_ms);
            if sender.capacity() == 0 {
                producer_state.live.lock().expect("state lock").waits += 1;
            }
            tokio::select! {biased;_ = producer_cancel.changed()=>{break},result=sender.send(event)=>{if result.is_err(){break}}}
            let occupancy = CAPACITY - sender.capacity();
            let mut l = producer_state.live.lock().expect("state lock");
            l.peak = l.peak.max(occupancy);
        }
    });
    tokio::spawn(async move {
        loop {
            tokio::select! {biased;_ = cancelled.changed()=>{break},event=receiver.recv()=>{match event {Some(e)=>{state.live.lock().expect("state lock").engine.process(e);},None=>break}}}
            tokio::task::yield_now().await;
        }
        drop(receiver);
        let _ = producer.await;
        let mut l = state.live.lock().expect("state lock");
        l.elapsed = l.start.map_or(0.0, |t| t.elapsed().as_secs_f64());
        l.phase = if *cancelled.borrow() {
            "stopped"
        } else {
            "complete"
        };
        l.cancel = None;
    });
    Ok(Json(json!({"started":true})))
}
async fn stop(State(s): State<AppState>) -> Json<Value> {
    let l = s.live.lock().expect("state lock");
    if let Some(cancel) = &l.cancel {
        let _ = cancel.send(true);
    }
    Json(json!({"stop_requested":true}))
}
async fn status(State(s): State<AppState>) -> Json<View> {
    Json(s.view())
}
async fn report(State(s): State<AppState>) -> Response {
    let mut r = Json(s.view()).into_response();
    r.headers_mut().insert(
        "content-disposition",
        HeaderValue::from_static("attachment; filename=sentinelflow-report.json"),
    );
    r
}
async fn research(State(s): State<AppState>) -> ApiResult {
    s.research.map(Json).ok_or((
        StatusCode::NOT_FOUND,
        "Research report unavailable; run the research executable and restart".into(),
    ))
}
async fn guard(State(s): State<AppState>, req: Request, next: Next) -> Response {
    let host = req
        .headers()
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");
    if !matches!(host, "localhost" | "127.0.0.1") {
        return (StatusCode::FORBIDDEN, "Local Host header required").into_response();
    }
    if req.uri().path().starts_with("/api/") {
        {
            let mut budget = s.budget.lock().expect("budget lock");
            if budget.0.elapsed() >= Duration::from_secs(1) {
                *budget = (Instant::now(), 0)
            }
            budget.1 += 1;
            if budget.1 > 120 {
                return (StatusCode::TOO_MANY_REQUESTS, "Request budget exceeded").into_response();
            }
        }
        if req.method() == "POST"
            && req
                .headers()
                .get("x-sentinelflow")
                .and_then(|v| v.to_str().ok())
                != Some("1")
        {
            return (StatusCode::FORBIDDEN, "X-SentinelFlow: 1 required").into_response();
        }
    }
    let mut response = next.run(req).await;
    let h = response.headers_mut();
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    h.insert("cache-control", HeaderValue::from_static("no-store"));
    h.insert("content-security-policy",HeaderValue::from_static("default-src 'self'; style-src 'self' 'unsafe-inline'; frame-ancestors 'none'; base-uri 'none'"));
    response
}
pub fn router(state: AppState) -> Router {
    Router::new()
        .route(
            "/api/health",
            get(|| async { Json(json!({"status":"ok"})) }),
        )
        .route("/api/status", get(status))
        .route(
            "/api/metrics",
            get(|State(s): State<AppState>| async move { Json(s.view().snapshot.metrics) }),
        )
        .route(
            "/api/alerts",
            get(|State(s): State<AppState>| async move { Json(s.view().snapshot.alerts) }),
        )
        .route(
            "/api/timeline",
            get(|State(s): State<AppState>| async move { Json(s.view().snapshot.timeline) }),
        )
        .route("/api/reports", get(report))
        .route("/api/research", get(research))
        .route("/api/replay", post(replay))
        .route("/api/stop", post(stop))
        .fallback_service(ServeDir::new("frontend/dist"))
        .layer(DefaultBodyLimit::max(16384))
        .layer(middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state)
}
pub async fn serve(port: u16) -> Result<(), String> {
    let research = std::fs::File::open("reports/research.json")
        .ok()
        .and_then(|f| {
            let mut data = Vec::new();
            f.take(8_388_609).read_to_end(&mut data).ok()?;
            if data.len() > 8_388_608 {
                return None;
            }
            serde_json::from_slice(&data).ok()
        });
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .map_err(|e| e.to_string())?;
    eprintln!("SentinelFlow listening on http://127.0.0.1:{port}");
    axum::serve(listener, router(AppState::new(research)))
        .await
        .map_err(|e| e.to_string())
}
/// Actual bounded channel, intentionally slow consumer; no hypothetical queue model.
pub async fn queue_probe() -> Value {
    let (tx, mut rx) = mpsc::channel(CAPACITY);
    let producer = tokio::spawn(async move {
        let mut waits = 0;
        let mut peak = 0;
        for i in 0..822 {
            if tx.capacity() == 0 {
                waits += 1;
            }
            tx.send(generator::workload(i, 42))
                .await
                .expect("consumer alive");
            peak = peak.max(CAPACITY - tx.capacity());
        }
        (waits, peak)
    });
    let start = Instant::now();
    let mut engine = Engine::new(Config::default()).expect("default valid");
    while let Some(e) = rx.recv().await {
        engine.process(e);
        tokio::time::sleep(Duration::from_micros(50)).await;
    }
    let (waits, peak) = producer.await.expect("producer task");
    let accepted = engine.snapshot().metrics.accepted;
    json!({"events":822,"accepted":accepted,"capacity":CAPACITY,"peak_observed":peak,"producer_waits":waits,"elapsed_ms":start.elapsed().as_secs_f64()*1000.0,"passed":accepted==822&&peak<=CAPACITY&&waits>0})
}
