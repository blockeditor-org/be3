//! lint-gate: fails on what clippy found. The prelude's clippy actions always
//! succeed and write their diagnostics as JSON lines (the `[clippy.json]`
//! subtarget), so buck/dev/broad.bxl hands every one of those files to this
//! action, which writes each warning and error, rendered, to SUMMARY, prints
//! them, and fails if there are any.

use std::process::ExitCode;

use serde_json::Value;

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let [summary, diagnostics @ ..] = arguments else {
        return Err("usage: lint-gate SUMMARY DIAG.JSON...".into());
    };
    let mut found: Vec<String> = Vec::new();
    for path in diagnostics {
        for finding in findings(&crate::read(path)?).map_err(|error| format!("{path}: {error}"))? {
            if !found.contains(&finding) {
                found.push(finding);
            }
        }
    }
    let mut text = found.concat();
    text.push_str(&format!(
        "lint-gate: {} findings in {} files\n",
        found.len(),
        diagnostics.len()
    ));
    std::fs::write(summary, &text).map_err(|error| format!("{summary}: {error}"))?;
    if found.is_empty() {
        return Ok(ExitCode::SUCCESS);
    }
    eprint!("{text}");
    Ok(ExitCode::FAILURE)
}

/// The rendered warnings and errors in one file of rustc's JSON diagnostics,
/// leaving out the totals rustc ends with ("aborting due to 2 previous
/// errors", "3 warnings emitted").
pub fn findings(text: &str) -> Result<Vec<String>, String> {
    let mut found = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let diagnostic: Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
        let level = diagnostic["level"].as_str().unwrap_or_default();
        if level != "error" && level != "warning" {
            continue;
        }
        let message = diagnostic["message"].as_str().unwrap_or_default();
        if message.starts_with("aborting due to") || message.ends_with(" emitted") {
            continue;
        }
        let rendered = diagnostic["rendered"].as_str().unwrap_or(message);
        found.push(format!("{}\n", rendered.trim_end()));
    }
    Ok(found)
}
