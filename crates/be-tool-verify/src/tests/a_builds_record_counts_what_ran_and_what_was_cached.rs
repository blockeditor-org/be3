use super::super::stats::{Context, record};

#[test]
fn a_builds_record_counts_what_ran_and_what_was_cached() {
    let action = |kind: u64, category: &str, name: &str, wall: u64| {
        format!(
            r#"{{"Event":{{"timestamp":[100,0],"trace_id":"t","data":{{"SpanEnd":{{"data":{{"ActionExecution":{{"key":{{"owner":{{"TargetLabel":{{"label":{{"package":"root//crates/a","name":"{name}"}}}}}}}},"name":{{"category":"{category}","identifier":""}},"execution_kind":{kind},"wall_time_us":{wall},"failed":false,"commands":[{{"details":{{"metadata":{{"queue_duration_us":2000000}}}}}}]}}}}}}}}}}}}"#
        )
    };
    let events = [
        r#"{"command_line_args":["buck2"]}"#.to_owned(),
        r#"{"Event":{"timestamp":[100,500000000],"trace_id":"t","data":{"SpanStart":{}}}}"#.to_owned(),
        action(2, "test_run", "test", 3_000_000),
        action(2, "rustc", "quick", 1_000_000),
        action(2, "rustc", "slow", 12_000_000),
        action(3, "rustc", "cached", 0),
        action(4, "write", "simple", 0),
        r#"{"Event":{"timestamp":[150,0],"trace_id":"t","data":{"Instant":{"data":{"BuildGraphInfo":{"critical_path2":[{"duration_us":12000000,"queue_duration_us":2000000,"entry":{"ActionExecution":{"name":{"category":"rustc","identifier":"link"},"owner":{"TargetLabel":{"label":{"package":"root//crates/a","name":"slow"}}}}}},{"duration_us":10,"entry":{"Waiting":{}}}]}}}}}}"#.to_owned(),
        r#"{"Event":{"timestamp":[160,500000000],"trace_id":"t","data":{"SpanEnd":{}}}}"#.to_owned(),
        r#"{"Result":{}}"#.to_owned(),
    ]
    .join("\n");
    let context = Context {
        kind: "verify".to_owned(),
        commit: Some("abc".to_owned()),
        branch: None,
        parts: "lint".to_owned(),
        passed: true,
    };
    let record = record(&events, &context).expect("a record");
    assert_eq!(record["id"], "t");
    assert_eq!(record["started"], 100.5);
    assert_eq!(record["seconds"], 60.0);
    assert_eq!(
        record["actions"],
        serde_json::json!({"total": 4, "cached": 1, "remote": 3, "local": 0})
    );
    assert_eq!(record["remote_seconds"], 16.0);
    assert_eq!(record["queue_seconds"], 6.0);
    let slow: Vec<&str> = record["slow"]
        .as_array()
        .unwrap()
        .iter()
        .map(|action| action["target"].as_str().unwrap())
        .collect();
    assert_eq!(slow, ["crates/a:test", "crates/a:slow"]);
    assert_eq!(
        record["critical_path"],
        serde_json::json!([{"what": "rustc link crates/a:slow", "seconds": 12.0, "queue_seconds": 2.0}])
    );
}
