use std::collections::{BTreeMap, BTreeSet};
use std::process::{Command, ExitCode};

use serde_json::Value;

type Edit = (String, u64, u64, String);

type Configuration = (Option<&'static str>, Vec<String>);

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let Some((bazel, flags)) = arguments.split_first() else {
        return Err(USAGE.into());
    };
    let mut fixing = false;
    let mut lints = true;
    let mut passed = Vec::new();
    for flag in flags {
        match flag.as_str() {
            "--fix" => fixing = true,
            "--no-lints" => lints = false,
            other if other.starts_with("--") => passed.push(other.to_owned()),
            _ => return Err(USAGE.into()),
        }
    }
    let clippy = Clippy {
        bazel,
        passed: &passed,
        lints,
    };
    let (mut built, mut found) = clippy.diagnostics()?;
    if fixing && !found.is_empty() {
        let applied = fix(&found)?;
        if applied > 0 {
            println!("Applied the fixes of {applied} clippy findings.");
            (built, found) = clippy.diagnostics()?;
        }
    }
    let count = report(&found);
    if count > 0 {
        println!("clippy: {count} findings.");
        return Ok(ExitCode::FAILURE);
    }
    if !built {
        println!("clippy: the build failed; its errors are above.");
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}

const USAGE: &str = "usage: clippy BAZEL [--fix] [--no-lints] [--BAZEL-FLAG...]";

struct Clippy<'a> {
    bazel: &'a str,
    passed: &'a [String],
    lints: bool,
}

impl Clippy<'_> {
    fn bazel(&self, command: &str) -> Command {
        let mut bazel = Command::new(self.bazel);
        bazel.arg(command).args(self.passed);
        bazel
    }

    fn configurations(&self) -> Result<Vec<Configuration>, String> {
        let wasm = crate::output(self.bazel("query").args([
            "--output=label",
            r#"attr(target_compatible_with, "wasm32", kind("rust_", //crates/...))"#,
        ]))?;
        let (guest, other): (Vec<String>, Vec<String>) =
            wasm.lines().map(str::to_owned).partition(|label| {
                label.starts_with("//crates/editors/")
                    || label.starts_with("//crates/block-editor-plugin:")
            });
        Ok(vec![
            (None, vec!["//crates/...".to_owned()]),
            (Some("//bazel/platforms:wasi_guest"), guest),
            (Some("//bazel/platforms:wasm32"), other),
            (
                Some("//bazel/platforms:wasi"),
                vec!["//crates/block-app:block-app-cdylib".to_owned()],
            ),
        ])
    }

    fn diagnostics(&self) -> Result<(bool, Vec<Value>), String> {
        let root = crate::output(self.bazel("info").arg("execution_root"))?;
        let root = root.trim();
        let events =
            std::env::temp_dir().join(format!("clippy-events-{}.json", std::process::id()));
        let flags = if self.lints {
            "--@rules_rust//rust/settings:clippy_flags=-Dwarnings,-Aclippy::too_many_arguments"
        } else {
            "--@rules_rust//rust/settings:clippy_flags=-Aclippy::all"
        };
        let mut built = true;
        let mut found = Vec::new();
        for (platform, targets) in self.configurations()? {
            if targets.is_empty() {
                continue;
            }
            let mut build = self.bazel("build");
            build.args([
                "--keep_going",
                "--aspects=@rules_rust//rust:defs.bzl%rust_clippy_aspect",
                "--output_groups=clippy_output",
                "--@rules_rust//rust/settings:clippy_output_diagnostics=true",
                "--@rules_rust//rust/settings:clippy.toml=//:clippy.toml",
                flags,
            ]);
            build.arg(format!("--build_event_json_file={}", events.display()));
            if let Some(platform) = platform {
                build.arg(format!("--platforms={platform}"));
            }
            build.arg("--").args(&targets);
            let status = build
                .status()
                .map_err(|error| format!("{}: {error}", self.bazel))?;
            built &= status.success();
            let text = crate::read(&events.to_string_lossy())?;
            for path in diagnostic_files(&text) {
                let path = format!("{root}/{path}");
                let Ok(contents) = std::fs::read_to_string(&path) else {
                    continue;
                };
                for entry in contents
                    .lines()
                    .map(str::trim)
                    .filter(|entry| entry.starts_with('{'))
                {
                    found.push(
                        serde_json::from_str(entry).map_err(|error| format!("{path}: {error}"))?,
                    );
                }
            }
        }
        let _ = std::fs::remove_file(&events);
        Ok((built, found))
    }
}

fn diagnostic_files(events: &str) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for line in events.lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        for file in event["namedSetOfFiles"]["files"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let Some(name) = file["name"].as_str() else {
                continue;
            };
            if !name.ends_with(".clippy.diagnostics") {
                continue;
            }
            let mut parts: Vec<&str> = file["pathPrefix"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            parts.push(name);
            paths.insert(parts.join("/"));
        }
    }
    paths
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
