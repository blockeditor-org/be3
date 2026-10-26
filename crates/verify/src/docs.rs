use std::fs;
use std::path::{Path, PathBuf};

pub fn painting_references(markdown: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for (number, line) in markdown.lines().enumerate() {
        let trimmed = line.trim_start();
        let marker = trimmed
            .chars()
            .next()
            .filter(|each| matches!(each, '`' | '~'));
        let run = marker.map_or(0, |marker| {
            trimmed.chars().take_while(|each| *each == marker).count()
        });
        match fence {
            Some((open, length)) => {
                if marker == Some(open) && run >= length && trimmed.trim_end().len() == run {
                    fence = None;
                }
                continue;
            }
            None if run >= 3 => {
                fence = marker.map(|marker| (marker, run));
                continue;
            }
            None => {}
        }
        for (index, part) in line.split('`').enumerate() {
            if index % 2 == 1 {
                continue;
            }
            let mut rest = part;
            while let Some(start) = rest.find("](paint:") {
                rest = &rest[start + "](paint:".len()..];
                let end = rest.find(')').unwrap_or(rest.len());
                let name = rest[..end].split('#').next().unwrap_or_default();
                found.push((number + 1, name.to_owned()));
                rest = &rest[end..];
            }
        }
    }
    found
}

pub fn missing_paintings(guides: &Path, snapshots: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    markdown_files(guides, &mut paths);
    paths.sort();
    let mut missing = Vec::new();
    for path in paths {
        let Ok(markdown) = fs::read_to_string(&path) else {
            continue;
        };
        for (line, name) in painting_references(&markdown) {
            if !snapshots.join(format!("{name}.paint")).exists() {
                missing.push(format!("{}:{line}: snapshots/{name}.paint", path.display()));
            }
        }
    }
    missing
}

fn markdown_files(directory: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            markdown_files(&path, paths);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            paths.push(path);
        }
    }
}
