use std::collections::BTreeSet;
use std::path::Path;
use std::process::ExitCode;

use crate::{Result, buck, repository};

const USAGE: &str = "Usage: ./scripts/test NAME [--update]\n       ./scripts/test //crates/<crate>:test [NAME] [--update]";

pub fn main(arguments: Vec<String>) -> Result<ExitCode> {
    let mut name = String::new();
    let mut update = false;
    let mut targets = Vec::new();
    for argument in arguments {
        if argument == "--update" {
            update = true;
        } else if argument.starts_with("//") {
            targets.push(argument);
        } else if argument.starts_with('-') || !name.is_empty() {
            return Err(USAGE.to_owned());
        } else {
            name = argument;
        }
    }
    if name.is_empty() && targets.is_empty() {
        return Err(USAGE.to_owned());
    }

    if targets.is_empty() {
        if !name.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == ':'
        }) {
            return Err(format!(
                "A test name is letters, digits, underscores and ::, not '{name}'."
            ));
        }
        let crates = crates_with(&name);
        if crates.is_empty() {
            return Err(format!(
                "No test in crates/ has a name containing '{name}'.\n\
                 A test's name is its module path and its function, such as media::tests::plays::plays for\n\
                 crates/block-app/src/media/tests/plays.rs; NAME may be any part of one."
            ));
        }
        for krate in crates {
            let rules =
                std::fs::read_to_string(repository().join(&krate).join("BUCK")).unwrap_or_default();
            let has_test = rules.lines().any(|line| {
                ["cargo_test(", "editor(", "plugin_tests("]
                    .iter()
                    .any(|rule| line.starts_with(rule))
            });
            if !has_test {
                eprintln!(
                    "Skipping {krate}: it holds a test whose name contains '{name}' but has no :test target in its BUCK."
                );
                continue;
            }
            targets.push(format!("//{krate}:test"));
        }
        if targets.is_empty() {
            return Err(String::new());
        }
    }

    let mut arguments = vec!["test".to_owned()];
    arguments.extend(targets.iter().cloned());
    arguments.extend(["--".to_owned(), "--test-arg=--nocapture".to_owned()]);
    if !name.is_empty() {
        arguments.push(format!("--test-arg={name}"));
    }
    if update {
        arguments.extend(["--env".to_owned(), "UPDATE_SNAPSHOTS=1".to_owned()]);
    }
    eprintln!("Running {}", targets.join(" "));
    // unsafe(env)
    unsafe { std::env::set_var("BE3_VERBOSE", "1") };
    buck::main(arguments)
}

fn crates_with(name: &str) -> BTreeSet<String> {
    let mut files = Vec::new();
    collect_rust_files(&repository().join("crates"), &mut files);
    let mut crates = BTreeSet::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        if !text.contains("#[test") && !text.contains("#[tokio::test") {
            continue;
        }
        let Ok(relative) = file.strip_prefix(repository()) else {
            continue;
        };
        let relative = relative.to_string_lossy().replace('\\', "/");
        for function in function_names(&text) {
            if let Some(krate) = crate_of_function(&relative, function, name) {
                crates.insert(krate);
            }
        }
    }
    crates
}

fn collect_rust_files(directory: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn function_names(text: &str) -> impl Iterator<Item = &str> {
    text.match_indices("fn ").filter_map(|(at, _)| {
        let rest = &text[at + 3..];
        let length = rest
            .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .unwrap_or(rest.len());
        (length > 0).then(|| &rest[..length])
    })
}

pub fn crate_of_function(file: &str, function: &str, name: &str) -> Option<String> {
    let (krate, mut relative) = if let Some(at) = file.find("/src/") {
        (&file[..at], file[at + 5..].to_owned())
    } else {
        let at = ["/tests/", "/examples/", "/benches/"]
            .iter()
            .filter_map(|marker| file.find(marker).map(|at| (at, marker.len())))
            .min()?;
        let after = &file[at.0 + at.1..];
        let after = match after.find('/') {
            Some(slash) => &after[slash + 1..],
            None => "",
        };
        (&file[..at.0], after.to_owned())
    };
    if let Some(stripped) = relative.strip_suffix(".rs") {
        relative = stripped.to_owned();
    }
    if let Some(rest) = relative.strip_prefix("bin/") {
        relative = match rest.find('/') {
            Some(slash) => rest[slash + 1..].to_owned(),
            None => String::new(),
        };
    }
    if relative == "mod" {
        relative.clear();
    } else if let Some(stripped) = relative.strip_suffix("/mod") {
        relative = stripped.to_owned();
    }
    if relative == "lib" || relative == "main" {
        relative.clear();
    }
    let relative = relative.replace('/', "::");
    let full = if relative.is_empty() {
        function.to_owned()
    } else {
        format!("{relative}::{function}")
    };
    full.contains(name).then(|| krate.to_owned())
}
