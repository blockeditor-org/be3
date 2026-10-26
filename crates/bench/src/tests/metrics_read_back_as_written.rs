use super::*;

#[test]
fn metrics_read_back_as_written() {
    let path = std::env::temp_dir().join(format!("bench-test-{}.metrics", std::process::id()));
    let _ = std::fs::remove_file(&path);
    // unsafe(env)
    unsafe { std::env::set_var(METRICS, &path) };
    time("open", Duration::from_micros(1500));
    count("allocations", 12);
    let text = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        parse(&text).unwrap(),
        vec![
            Metric {
                name: "open".to_owned(),
                value: 1.5,
                unit: "ms".to_owned()
            },
            Metric {
                name: "allocations".to_owned(),
                value: 12.0,
                unit: String::new()
            },
        ]
    );
    assert!(parse("open\tfast\tms").is_err());
    assert_eq!(suite::median(&[3.0, 1.0, 2.0, 10.0]), 2.5);
    assert_eq!(suite::fastest(&[3.0, 1.0, 2.0]), 1.0);
}
