use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use sentinelflow_ids::api::{AppState, router};
use serde_json::{Value, json};
use tower::ServiceExt;
fn request(path: &str, body: Option<Value>, header: bool) -> Request<Body> {
    let mut r = Request::builder()
        .uri(path)
        .header("host", "127.0.0.1:8787");
    if header {
        r = r.header("x-sentinelflow", "1")
    }
    if let Some(v) = body {
        r.method("POST")
            .header("content-type", "application/json")
            .body(Body::from(v.to_string()))
            .unwrap()
    } else {
        r.body(Body::empty()).unwrap()
    }
}
#[tokio::test]
async fn replay_uses_real_engine_and_reports() {
    let state = AppState::default();
    let app = router(state.clone());
    let r = app
        .clone()
        .oneshot(request(
            "/api/replay",
            Some(json!({"scenario":"mixed","seed":42,"speed":0})),
            true,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while state.view().phase == "running" {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let v = state.view();
    assert_eq!(v.snapshot.metrics.accepted, 822);
    assert_eq!(v.snapshot.metrics.alerts_total, 4);
    let r = app
        .oneshot(request("/api/reports", None, false))
        .await
        .unwrap();
    assert!(
        r.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .contains("sentinelflow-report.json")
    );
    let b = to_bytes(r.into_body(), 1_000_000).await.unwrap();
    let report: Value = serde_json::from_slice(&b).unwrap();
    assert_eq!(report["snapshot"]["metrics"]["accepted"], 822);
}
#[tokio::test]
async fn rejects_unsafe_requests() {
    let app = router(AppState::default());
    for (v, header, expected) in [
        (json!({"scenario":"mixed","seed":42,"speed":0}), false, 403),
        (json!({"scenario":"unknown","seed":42,"speed":0}), true, 400),
        (
            json!({"scenario":"mixed","seed":42,"speed":0,"path":"/etc/passwd"}),
            true,
            422,
        ),
        (
            json!({"scenario":"mixed","seed":42,"speed":0,"config":{"window_ms":0}}),
            true,
            400,
        ),
    ] {
        assert_eq!(
            app.clone()
                .oneshot(request("/api/replay", Some(v), header))
                .await
                .unwrap()
                .status()
                .as_u16(),
            expected
        );
    }
    let req = Request::builder()
        .uri("/api/status")
        .header("host", "evil.example")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.oneshot(req).await.unwrap().status(), 403);
}
#[tokio::test]
async fn stop_and_busy_are_real() {
    let state = AppState::default();
    let app = router(state.clone());
    let body = json!({"scenario":"mixed","seed":42,"speed":1});
    assert_eq!(
        app.clone()
            .oneshot(request("/api/replay", Some(body.clone()), true))
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        app.clone()
            .oneshot(request("/api/replay", Some(body), true))
            .await
            .unwrap()
            .status(),
        409
    );
    app.oneshot(request("/api/stop", Some(json!({})), true))
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while state.view().phase == "running" {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(state.view().phase, "stopped");
    assert!(state.view().snapshot.metrics.accepted < 822);
}
#[tokio::test]
async fn slow_queue_and_request_caps() {
    let probe = sentinelflow_ids::api::queue_probe().await;
    assert_eq!(probe["passed"], true);
    let app = router(AppState::default());
    let big = json!({"scenario":"x".repeat(17000),"seed":1,"speed":0});
    assert_eq!(
        app.clone()
            .oneshot(request("/api/replay", Some(big), true))
            .await
            .unwrap()
            .status(),
        413
    );
    let mut last = 200;
    for _ in 0..125 {
        last = app
            .clone()
            .oneshot(request("/api/status", None, false))
            .await
            .unwrap()
            .status()
            .as_u16();
    }
    assert_eq!(last, 429);
}
