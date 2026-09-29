use std::process::ExitCode;

use serde_json::Value;

const MARKER: &str = "//~ ERROR ";

struct Located {
    file: String,
    line: u64,
    message: String,
}

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let [diagnostics, sources @ ..] = arguments else {
        return Err("usage: compile-fail DIAG.JSON SOURCE...".into());
    };
    let expected = expectations(sources)?;
    let found = errors(diagnostics)?;
    let mut failures = Vec::new();
    for expectation in &expected {
        if !found.iter().any(|error| matches(error, expectation)) {
            failures.push(format!(
                "{}:{}: expected an error containing {:?}, and rustc reported none there",
                expectation.file, expectation.line, expectation.message
            ));
        }
    }
    for error in &found {
        if !expected
            .iter()
            .any(|expectation| matches(error, expectation))
        {
            failures.push(format!(
                "{}:{}: unexpected error: {}",
                error.file, error.line, error.message
            ));
        }
    }
    if failures.is_empty() {
        println!(
            "compile-fail: all {} expected errors reported",
            expected.len()
        );
        return Ok(ExitCode::SUCCESS);
    }
    for failure in &failures {
        println!("{failure}");
    }
    Ok(ExitCode::FAILURE)
}

fn matches(error: &Located, expectation: &Located) -> bool {
    error.file == expectation.file
        && error.line == expectation.line
        && error.message.contains(&expectation.message)
}

fn expectations(sources: &[String]) -> Result<Vec<Located>, String> {
    let mut expected = Vec::new();
    for source in sources {
        for (index, line) in crate::read(source)?.lines().enumerate() {
            if let Some((_, message)) = line.split_once(MARKER) {
                expected.push(Located {
                    file: source.clone(),
                    line: index as u64 + 1,
                    message: message.trim().to_string(),
                });
            }
        }
    }
    Ok(expected)
}

fn errors(path: &str) -> Result<Vec<Located>, String> {
    let mut found = Vec::new();
    for entry in crate::read(path)?
        .lines()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let diagnostic: Value =
            serde_json::from_str(entry).map_err(|error| format!("{path}: {error}"))?;
        if diagnostic["level"] != "error" {
            continue;
        }
        let primary = diagnostic["spans"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|span| span["is_primary"] == true);
        let Some(span) = primary else {
            continue;
        };
        found.push(Located {
            file: span["file_name"].as_str().unwrap_or_default().to_string(),
            line: span["line_start"].as_u64().unwrap_or_default(),
            message: diagnostic["message"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        });
    }
    Ok(found)
}
