use super::*;

#[test]
fn lint_gate_keeps_warnings_and_errors_but_not_totals() {
    let lines = [
        json!({"level": "error", "message": "this `if` has identical blocks", "rendered": "error: this `if` has identical blocks\n --> src/lib.rs:3:5\n"}),
        json!({"level": "warning", "message": "unused import", "rendered": "warning: unused import\n"}),
        json!({"level": "note", "message": "a note", "rendered": "note: a note\n"}),
        json!({"level": "error", "message": "aborting due to 1 previous error", "rendered": "error: aborting due to 1 previous error\n"}),
        json!({"level": "warning", "message": "1 warning emitted", "rendered": "warning: 1 warning emitted\n"}),
    ];
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();

    let found = crate::lint_gate::findings(&text).unwrap();

    assert_eq!(
        found,
        [
            "error: this `if` has identical blocks\n --> src/lib.rs:3:5\n",
            "warning: unused import\n",
        ]
    );
    assert!(crate::lint_gate::findings("").unwrap().is_empty());
}
