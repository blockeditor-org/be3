use super::*;

#[test]
fn quiet_keeps_what_explains_a_failure() {
    let log = "\
Build ID: 1234
[2026-10-20T00:00:00.000] Waiting on root//crates/a:a -- action (rustc)
[a] ✓ Pass: root//crates/a:test (0.1s)
[a] test passes ... ok
[b] ✗ Fail: root//crates/b:test (0.2s)
[b] test fails ... FAILED
Skipped 2 incompatible targets:
  root//crates/c:test
  root//crates/d:test
error[E0308]: mismatched types
BUILD SUCCEEDED
";
    let mut out = Vec::new();
    let mut quiet = Quiet::new(false, 80);
    for line in log.lines() {
        quiet.line(line, &mut out);
    }
    quiet.finish(&mut out);
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "✗ Fail: root//crates/b:test (0.2s)\ntest fails ... FAILED\nerror[E0308]: mismatched types\n"
    );
}
