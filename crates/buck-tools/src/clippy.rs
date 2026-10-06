use std::collections::{BTreeMap, BTreeSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::process::ExitCode;

use serde_json::Value;

type Edit = (String, u64, u64, String);

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let [root, findings, diagnostics @ ..] = arguments else {
        return Err("usage: clippy ROOT FINDINGS CLIPPY_JSON...".into());
    };
    let mut found = Vec::new();
    for path in diagnostics {
        for entry in crate::read(path)?
            .lines()
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
        {
            found.push(serde_json::from_str(entry).map_err(|error| format!("{path}: {error}"))?);
        }
    }
    let applied = fix(Path::new(root), &found)?;
    let remaining: Vec<Value> = found
        .into_iter()
        .filter(|diagnostic| !applied.contains(&suggestions(diagnostic)))
        .collect();
    write_findings(Path::new(findings), &remaining)?;
    Ok(ExitCode::SUCCESS)
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

fn fix(root: &Path, found: &[Value]) -> Result<BTreeSet<BTreeSet<Edit>>, String> {
    let mut chosen: BTreeMap<String, Vec<(u64, u64, String)>> = BTreeMap::new();
    let mut applied = BTreeSet::new();
    for diagnostic in found {
        let edits = suggestions(diagnostic);
        if edits.is_empty()
            || !edits
                .iter()
                .all(|edit| edit.0.starts_with("crates/") && root.join(&edit.0).is_file())
            || applied.contains(&edits)
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
        for (path, start, end, replacement) in edits.iter().cloned() {
            chosen
                .entry(path)
                .or_default()
                .push((start, end, replacement));
        }
        applied.insert(edits);
    }
    for (path, mut edits) in chosen {
        let path = root.join(path);
        let mut source =
            std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        edits.sort();
        for (start, end, replacement) in edits.into_iter().rev() {
            source.splice(start as usize..end as usize, replacement.into_bytes());
        }
        std::fs::write(&path, source).map_err(|error| format!("{}: {error}", path.display()))?;
    }
    Ok(applied)
}

fn write_findings(directory: &Path, found: &[Value]) -> Result<(), String> {
    std::fs::create_dir_all(directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?;
    for diagnostic in found {
        let level = diagnostic["level"].as_str().unwrap_or_default();
        if !matches!(level, "error" | "warning") || !first_party(diagnostic) {
            continue;
        }
        let text = diagnostic["rendered"]
            .as_str()
            .or_else(|| diagnostic["message"].as_str())
            .unwrap_or_default();
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let path = directory.join(format!("{:016x}.txt", hasher.finish()));
        std::fs::write(&path, text).map_err(|error| format!("{}: {error}", path.display()))?;
    }
    Ok(())
}
