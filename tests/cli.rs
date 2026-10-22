use std::{fs, process::Command};
fn temp(name: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("sentinelflow-{name}-{}", std::process::id()));
    fs::create_dir(&p).unwrap();
    p
}
#[test]
fn generated_demo_and_no_overwrite() {
    let p = temp("demo");
    let out = p.join("fixture");
    let bin = env!("CARGO_BIN_EXE_sentinelflow-ids");
    assert!(
        Command::new(bin)
            .args(["generate", "mixed", "42"])
            .arg(&out)
            .status()
            .unwrap()
            .success()
    );
    let result = Command::new(bin)
        .arg("replay")
        .arg(out.join("events.jsonl"))
        .arg("--output")
        .arg(out.join("report.json"))
        .output()
        .unwrap();
    assert!(result.status.success());
    let v: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(v["snapshot"]["metrics"]["accepted"], 822);
    assert!(
        !Command::new(bin)
            .args(["generate", "mixed", "42"])
            .arg(&out)
            .status()
            .unwrap()
            .success()
    );
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn invalid_record_produces_visible_partial_report() {
    let p = temp("invalid");
    fs::write(p.join("bad.jsonl"), "invalid\n").unwrap();
    let r = Command::new(env!("CARGO_BIN_EXE_sentinelflow-ids"))
        .arg("replay")
        .arg(p.join("bad.jsonl"))
        .output()
        .unwrap();
    assert_eq!(r.status.code(), Some(3));
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(v["snapshot"]["metrics"]["invalid"], 1);
    assert!(String::from_utf8_lossy(&r.stderr).contains("Line 1"));
    fs::remove_dir_all(p).unwrap();
}
