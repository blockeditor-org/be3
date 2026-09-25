use std::collections::{BTreeMap, BTreeSet};
use std::process::{Command, ExitCode};

use serde_json::Value;

type Edit = (String, u64, u64, String);

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let (buck, fixing) = match arguments {
        [buck] => (buck, false),
        [buck, flag] if flag == "--fix" => (buck, true),
        _ => return Err("usage: clippy BUCK [--fix]".into()),
    };
    let mut found = diagnostics(buck)?;
    if fixing && !found.is_empty() {
        let applied = fix(&found)?;
        if applied > 0 {
            println!("Applied the fixes of {applied} clippy findings.");
            found = diagnostics(buck)?;
        }
    }
    let count = report(&found);
    if count > 0 {
        println!("clippy: {count} findings.");
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}

fn diagnostics(buck: &str) -> Result<Vec<Value>, String> {
    let listing = crate::output(Command::new(buck).args([
        "bxl",
        "//buck/dev/workspace.bxl:subtarget",
        "--",
        "--subtarget",
        "clippy.json",
    ]))?;
    let mut found = Vec::new();
    for line in listing.lines() {
        let Some((_, path)) = line.split_once('\t') else {
            continue;
        };
        for entry in crate::read(path)?
            .lines()
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
        {
            found.push(serde_json::from_str(entry).map_err(|error| format!("{path}: {error}"))?);
        }
    }
    Ok(found)
}

fn spans(diagnostic: &Value) -> impl Iterator<Item = &Value> {
    diagnostic["spans"].as_array().into_iter().flatten()
}

fn file_name(span: &Value) -> &str {
    span["file_name"].as_str().unwrap_or_default()
}

fn first_party(diagnostic: &Value) -> bool {
    spans(diagnostic).any(|span| file_name(span).starts_with("crates/"))
}

fn suggestions(diagnostic: &Value) -> BTreeSet<Edit> {
    let mut edits = BTreeSet::new();
    let mut pending = vec![diagnostic];
    while let Some(current) = pending.pop() {
        for span in spans(current) {
            let Some(replacement) = span["suggested_replacement"].as_str() else {
                continue;
            };
            if span["suggestion_applicability"] != "MachineApplicable" {
                continue;
            }
            edits.insert((
                file_name(span).to_owned(),
                span["byte_start"].as_u64().unwrap_or_default(),
                span["byte_end"].as_u64().unwrap_or_default(),
                replacement.to_owned(),
            ));
        }
        pending.extend(current["children"].as_array().into_iter().flatten());
    }
    edits
}

fn fix(found: &[Value]) -> Result<usize, String> {
    let mut chosen: BTreeMap<String, Vec<(u64, u64, String)>> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let mut applied = 0;
    for diagnostic in found {
        let edits = suggestions(diagnostic);
        if edits.is_empty()
            || !edits.iter().all(|edit| edit.0.starts_with("crates/"))
            || !seen.insert(edits.clone())
        {
            continue;
        }
        let overlaps = edits.iter().any(|(path, start, end, _)| {
            chosen
                .get(path)
                .into_iter()
                .flatten()
                .any(|(other_start, other_end, _)| start < other_end && other_start < end)
        });
        if overlaps {
            continue;
        }
        for (path, start, end, replacement) in edits {
            chosen
                .entry(path)
                .or_default()
                .push((start, end, replacement));
        }
        applied += 1;
    }
    for (path, mut edits) in chosen {
        let mut source = std::fs::read(&path).map_err(|error| format!("{path}: {error}"))?;
        edits.sort();
        for (start, end, replacement) in edits.into_iter().rev() {
            source.splice(start as usize..end as usize, replacement.into_bytes());
        }
        std::fs::write(&path, source).map_err(|error| format!("{path}: {error}"))?;
    }
    Ok(applied)
}

fn report(found: &[Value]) -> usize {
    let mut rendered: Vec<&str> = Vec::new();
    for diagnostic in found {
        let level = diagnostic["level"].as_str().unwrap_or_default();
        if !matches!(level, "error" | "warning") || !first_party(diagnostic) {
            continue;
        }
        let text = diagnostic["rendered"]
            .as_str()
            .or_else(|| diagnostic["message"].as_str())
            .unwrap_or_default();
        if !rendered.contains(&text) {
            rendered.push(text);
        }
    }
    for text in &rendered {
        print!("{text}");
    }
    rendered.len()
}
