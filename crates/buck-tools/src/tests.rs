use std::path::PathBuf;

use serde_json::{Value, json};

fn temporary_directory(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("buck-tools-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn unused_import(file: &str, start: u64, end: u64) -> Value {
    json!({
        "level": "error",
        "code": {"code": "unused_imports"},
        "message": "unused import",
        "rendered": "error: unused import",
        "spans": [{"file_name": file, "byte_start": start, "byte_end": end}],
        "children": [{
            "spans": [{
                "file_name": file,
                "byte_start": start,
                "byte_end": end,
                "suggested_replacement": "",
                "suggestion_applicability": "MachineApplicable",
            }],
            "children": [],
        }],
    })
}

fn run_clippy(name: &str, source: &str, diagnostics: &[Value]) -> String {
    let directory = temporary_directory(name);
    let root = directory.join("root");
    std::fs::create_dir_all(root.join("crates/demo")).unwrap();
    std::fs::write(root.join("crates/demo/lib.rs"), source).unwrap();
    let json = directory.join("clippy.json");
    let lines: Vec<String> = diagnostics.iter().map(Value::to_string).collect();
    std::fs::write(&json, lines.join("\n")).unwrap();
    crate::clippy::run(&[
        root.display().to_string(),
        directory.join("findings").display().to_string(),
        json.display().to_string(),
    ])
    .unwrap();
    let fixed = std::fs::read_to_string(root.join("crates/demo/lib.rs")).unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    fixed
}

mod clippy_applies_no_fix_to_a_crate_that_failed_to_compile;
mod clippy_removes_an_unused_import;
