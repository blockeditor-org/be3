use crate::parse;
use ra_ap_syntax::AstNode;
use ra_ap_syntax::ast::{self, HasName};
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

pub fn fix_docs(docs: &Path, code: &Path) -> Result<(), Box<dyn Error>> {
    let crates = crate_directories(code)?;
    let mut sources = BTreeMap::new();
    let mut problems = Vec::new();
    let mut paths = Vec::new();
    collect_markdown(docs, &mut paths)?;
    paths.sort();
    for path in paths {
        let markdown = fs::read_to_string(&path)?;
        let shown = path
            .strip_prefix(docs)
            .unwrap_or(&path)
            .display()
            .to_string();
        let mut lookup =
            |language: &str, id: &str| find_example(language, id, &crates, &mut sources);
        match sync_examples(&markdown, &mut lookup) {
            Ok(synced) if synced != markdown => fs::write(&path, synced)?,
            Ok(_) => {}
            Err(errors) => {
                problems.extend(errors.into_iter().map(|error| format!("{shown}:{error}")))
            }
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "code samples in the docs need fixing (guides/code_samples.md):\n{}",
            problems.join("\n")
        )
        .into())
    }
}

pub fn sync_examples(
    markdown: &str,
    lookup: &mut dyn FnMut(&str, &str) -> Result<String, String>,
) -> Result<String, Vec<String>> {
    let mut out = String::with_capacity(markdown.len());
    let mut errors = Vec::new();
    let mut lines = markdown.split_inclusive('\n').enumerate();
    while let Some((number, line)) = lines.next() {
        out.push_str(line);
        let Some(fence) = Fence::open(line) else {
            continue;
        };
        let mut body = Vec::new();
        let mut closing = None;
        for (_, line) in lines.by_ref() {
            if fence.closes(line) {
                closing = Some(line);
                break;
            }
            body.push(line);
        }
        let Some(closing) = closing else {
            body.iter().for_each(|line| out.push_str(line));
            break;
        };
        match fence.example() {
            None => body.iter().for_each(|line| out.push_str(line)),
            Some((language, id)) => match lookup(language, id) {
                Ok(code) => {
                    for code_line in code.lines() {
                        if !code_line.is_empty() {
                            out.push_str(&fence.indent);
                        }
                        out.push_str(code_line);
                        out.push('\n');
                    }
                }
                Err(error) => {
                    errors.push(format!("{}: {error}", number + 1));
                    body.iter().for_each(|line| out.push_str(line));
                }
            },
        }
        out.push_str(closing);
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

struct Fence {
    indent: String,
    marker: char,
    length: usize,
    info: String,
}

impl Fence {
    fn open(line: &str) -> Option<Self> {
        let text = line.trim_end_matches(['\n', '\r']);
        let rest = text.trim_start_matches(' ');
        let indent = &text[..text.len() - rest.len()];
        let marker = rest
            .chars()
            .next()
            .filter(|marker| matches!(marker, '`' | '~'))?;
        let length = rest.chars().take_while(|each| *each == marker).count();
        if length < 3 {
            return None;
        }
        let info = rest[length..].trim();
        if marker == '`' && info.contains('`') {
            return None;
        }
        Some(Self {
            indent: indent.to_owned(),
            marker,
            length,
            info: info.to_owned(),
        })
    }

    fn closes(&self, line: &str) -> bool {
        let rest = line.trim();
        rest.len() >= self.length && rest.chars().all(|each| each == self.marker)
    }

    fn example(&self) -> Option<(&str, &str)> {
        let mut words = self.info.split_whitespace();
        let language = words.next()?;
        let id = words.find_map(|word| word.strip_prefix("example="))?;
        Some((language, id))
    }
}

pub fn example_body(source: &str, name: &str) -> Result<Option<String>, Box<dyn Error>> {
    let tree = parse(source.as_bytes())?;
    let mut found = tree.syntax().descendants().filter_map(|node| {
        if let Some(function) = ast::Fn::cast(node.clone()) {
            return is_named(&function, name).then(|| {
                function
                    .body()
                    .and_then(|body| body.stmt_list())
                    .map(|statements| statements.syntax().text().to_string())
            });
        }
        let module = ast::Module::cast(node)?;
        is_named(&module, name).then(|| {
            module
                .item_list()
                .map(|items| items.syntax().text().to_string())
        })
    });
    let Some(braced) = found.next() else {
        return Ok(None);
    };
    if found.next().is_some() {
        return Err(format!("more than one fn or mod {name}").into());
    }
    let braced = braced.ok_or_else(|| format!("{name} has no body"))?;
    let inner = braced
        .strip_prefix('{')
        .and_then(|text| text.strip_suffix('}'))
        .unwrap_or(&braced);
    Ok(Some(dedented(inner)))
}

fn is_named(node: &impl HasName, name: &str) -> bool {
    node.name().is_some_and(|each| each.text() == name)
}

fn dedented(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let first = lines.iter().position(|line| !line.trim().is_empty());
    let last = lines.iter().rposition(|line| !line.trim().is_empty());
    let (Some(first), Some(last)) = (first, last) else {
        return String::new();
    };
    let lines = &lines[first..=last];
    let indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);
    let mut out = String::new();
    for line in lines {
        out.push_str(line.get(indent..).unwrap_or("").trim_end());
        out.push('\n');
    }
    out
}

fn find_example(
    language: &str,
    id: &str,
    crates: &BTreeMap<String, PathBuf>,
    sources: &mut BTreeMap<PathBuf, String>,
) -> Result<String, String> {
    if language != "rust" {
        return Err(format!(
            "examples can only be in rust so far, not {language}"
        ));
    }
    let (package, name) = id
        .split_once('.')
        .ok_or_else(|| format!("example={id} is not CRATE.NAME"))?;
    let directory = crates
        .get(package)
        .ok_or_else(|| format!("example={id}: no crate is named {package}"))?;
    let function = format!("example_{name}");
    let mut files = Vec::new();
    collect_crate_sources(directory, true, &mut files).map_err(|error| error.to_string())?;
    files.sort();
    let mut found = Vec::new();
    for file in files {
        if !sources.contains_key(&file) {
            let source = fs::read_to_string(&file).map_err(|error| error.to_string())?;
            sources.insert(file.clone(), source);
        }
        let body = example_body(&sources[&file], &function)
            .map_err(|error| format!("{}: {error}", file.display()))?;
        found.extend(body);
    }
    match found.len() {
        0 => Err(format!(
            "example={id}: crate {package} has no fn or mod {function}"
        )),
        1 => Ok(found.remove(0)),
        _ => Err(format!(
            "example={id}: crate {package} has more than one fn or mod {function}"
        )),
    }
}

fn crate_directories(code: &Path) -> Result<BTreeMap<String, PathBuf>, Box<dyn Error>> {
    let mut manifests = Vec::new();
    collect_named(code, "Cargo.toml", &mut manifests)?;
    let mut crates = BTreeMap::new();
    for manifest in manifests {
        let text = fs::read_to_string(&manifest)?;
        if let Some(name) = package_name(&text) {
            crates.insert(name, manifest.parent().unwrap().to_path_buf());
        }
    }
    Ok(crates)
}

fn package_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if in_package
            && let Some((key, value)) = line.split_once('=')
            && key.trim() == "name"
        {
            return Some(value.trim().trim_matches('"').to_owned());
        }
    }
    None
}

fn skipped(name: &str) -> bool {
    name.starts_with('.') || name == "target"
}

fn collect_named(
    directory: &Path,
    file: &str,
    paths: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            if !skipped(&entry.file_name().to_string_lossy()) {
                collect_named(&path, file, paths)?;
            }
        } else if entry.file_name() == file {
            paths.push(path);
        }
    }
    Ok(())
}

fn collect_crate_sources(
    directory: &Path,
    root: bool,
    paths: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    if !root && directory.join("Cargo.toml").exists() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            if !skipped(&entry.file_name().to_string_lossy()) {
                collect_crate_sources(&path, false, paths)?;
            }
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path);
        }
    }
    Ok(())
}

fn collect_markdown(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            if !skipped(&entry.file_name().to_string_lossy()) {
                collect_markdown(&path, paths)?;
            }
        } else if path.extension().is_some_and(|extension| extension == "md") {
            paths.push(path);
        }
    }
    Ok(())
}
