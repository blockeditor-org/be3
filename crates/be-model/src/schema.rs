use std::collections::{BTreeMap, BTreeSet};

use serde::{Serialize, de::DeserializeOwned};

use crate::{Stored, Value, stored};

pub type Convert = fn(&Stored) -> Option<Stored>;

pub type Migrate = fn(u32, &mut Stored);

pub struct Kind {
    pub name: &'static str,
    pub format: u32,
    pub migrate: Migrate,
    pub blank: fn() -> Vec<Value>,
    pub properties: Vec<Property>,
}

pub struct Property {
    pub name: &'static str,
    pub aliases: Vec<&'static str>,
    pub from_old: Vec<(&'static str, Convert)>,
    pub critical: bool,
    pub shape: Shape,
}

pub enum Shape {
    Register { accepts: fn(&[u8]) -> bool },
    Count,
    Text,
    List(fn() -> Kind),
    Map,
    Grid,
    Latest,
}

pub fn no_migration(_format: u32, _root: &mut Stored) {}

pub fn accepts<T: DeserializeOwned>(bytes: &[u8]) -> bool {
    stored::decode::<T>(bytes).is_some()
}

pub fn convert<A: DeserializeOwned, B: Serialize>(
    old: &Stored,
    change: fn(A) -> B,
) -> Option<Stored> {
    let old: A = old.deserialized().ok()?;
    Stored::serialized(&change(old)).ok()
}

impl Property {
    pub fn old_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.aliases
            .iter()
            .copied()
            .chain(self.from_old.iter().map(|(name, _)| *name))
    }
}

impl Kind {
    pub fn describe(&self) -> String {
        let mut lines = BTreeSet::new();
        let mut seen = BTreeSet::new();
        self.describe_into(&mut lines, &mut seen);
        let mut out = format!("format {}\n", self.format);
        for line in lines {
            out.push_str(&line);
            out.push('\n');
        }
        out
    }

    fn describe_into(&self, lines: &mut BTreeSet<String>, seen: &mut BTreeSet<&'static str>) {
        if !seen.insert(self.name) {
            return;
        }
        lines.insert(format!("kind {}", self.name));
        let blank = (self.blank)();
        for (index, property) in self.properties.iter().enumerate() {
            let path = format!("{}.{}", self.name, property.name);
            let shape = match &property.shape {
                Shape::Register { .. } => {
                    if let Some(default) = blank.get(index).and_then(|value| match value {
                        Value::Register(bytes) => stored::decode::<Stored>(bytes),
                        _ => None,
                    }) {
                        defaults(&default, &path, lines);
                    }
                    "register".to_owned()
                }
                Shape::Count => "count".to_owned(),
                Shape::Text => "text".to_owned(),
                Shape::List(kind) => format!("list {}", kind().name),
                Shape::Map => "map".to_owned(),
                Shape::Grid => "grid".to_owned(),
                Shape::Latest => "latest".to_owned(),
            };
            let critical = if property.critical { " critical" } else { "" };
            lines.insert(format!("property {path}: {shape}{critical}"));
            for old in property.old_names() {
                lines.insert(format!("old {}.{old} -> {}", self.name, property.name));
            }
            if let Shape::List(kind) = &property.shape {
                kind().describe_into(lines, seen);
            }
        }
    }

    pub fn registers_accept_missing_fields(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let mut seen = BTreeSet::new();
        self.check_registers(&mut problems, &mut seen);
        problems
    }

    fn check_registers(&self, problems: &mut Vec<String>, seen: &mut BTreeSet<&'static str>) {
        if !seen.insert(self.name) {
            return;
        }
        let blank = (self.blank)();
        for (index, property) in self.properties.iter().enumerate() {
            match &property.shape {
                Shape::Register { accepts } => {
                    let Some(Value::Register(bytes)) = blank.get(index) else {
                        continue;
                    };
                    let Some(default) = stored::decode::<Stored>(bytes) else {
                        continue;
                    };
                    for (path, trimmed) in without_each_field(&default, String::new()) {
                        if !accepts(&stored::encode(&trimmed)) {
                            problems.push(format!(
                                "{}.{} does not read without {path}; give its type #[serde(default)]",
                                self.name, property.name
                            ));
                        }
                    }
                }
                Shape::List(kind) => kind().check_registers(problems, seen),
                _ => {}
            }
        }
    }
}

fn defaults(value: &Stored, path: &str, lines: &mut BTreeSet<String>) {
    match value {
        Stored::Map(entries) if !entries.is_empty() => {
            for (key, inner) in entries {
                let name = key
                    .as_text()
                    .map_or_else(|| stored::diagnostic(key), str::to_owned);
                defaults(inner, &format!("{path}.{name}"), lines);
            }
        }
        _ => {
            lines.insert(format!("default {path} = {}", stored::diagnostic(value)));
        }
    }
}

fn without_each_field(value: &Stored, at: String) -> Vec<(String, Stored)> {
    let mut out = Vec::new();
    match value {
        Stored::Map(entries) => {
            let variant = entries.len() == 1 && entries.iter().all(|(_, inner)| inner.is_map());
            for (index, (key, inner)) in entries.iter().enumerate() {
                let name = key.as_text().unwrap_or("?");
                let path = if at.is_empty() {
                    name.to_owned()
                } else {
                    format!("{at}.{name}")
                };
                if !variant {
                    let mut trimmed = entries.clone();
                    trimmed.remove(index);
                    out.push((path.clone(), Stored::Map(trimmed)));
                }
                for (inner_path, inner_trimmed) in without_each_field(inner, path) {
                    let mut replaced = entries.clone();
                    replaced[index].1 = inner_trimmed;
                    out.push((inner_path, Stored::Map(replaced)));
                }
            }
        }
        Stored::Tag(tag, inner) => {
            for (path, trimmed) in without_each_field(inner, at) {
                out.push((path, Stored::Tag(*tag, Box::new(trimmed))));
            }
        }
        _ => {}
    }
    out
}

pub fn compatible(frozen: &str, current: &str) -> Result<(), Vec<String>> {
    let frozen = Registry::parse(frozen);
    let current = Registry::parse(current);
    let mut problems = Vec::new();
    if current.format < frozen.format {
        problems.push(format!(
            "the format went back from {} to {}",
            frozen.format, current.format
        ));
    }
    for kind in &frozen.kinds {
        if !current.kinds.contains(kind) {
            problems.push(format!("kind {kind} is gone"));
        }
    }
    let mut unchanged = BTreeSet::new();
    for (name, shape) in &frozen.properties {
        let still = current
            .properties
            .get(name)
            .map(|now| (name.clone(), now))
            .or_else(|| {
                current
                    .old
                    .get(name)
                    .and_then(|new| current.properties.get(new).map(|now| (new.clone(), now)))
            });
        match still {
            None => problems.push(format!(
                "property {name} is gone; keep reading it with #[model(alias)] or #[model(from_old)]"
            )),
            Some((now_name, now)) if now_name == *name => {
                if strip_critical(now) == strip_critical(shape) {
                    unchanged.insert(name.as_str());
                } else {
                    problems.push(format!("property {name} changed from `{shape}` to `{now}`"));
                }
            }
            Some(_) => {}
        }
    }
    for (path, value) in &frozen.defaults {
        let owner = unchanged
            .iter()
            .any(|property| path == property || path.starts_with(&format!("{property}.")));
        if !owner {
            continue;
        }
        match current.defaults.get(path) {
            Some(now) if now == value => {}
            Some(now) => problems.push(format!(
                "the default of {path} changed from {value} to {now}"
            )),
            None => problems.push(format!(
                "{path} is gone from its register; a field inside a register can never be removed or renamed"
            )),
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

fn strip_critical(shape: &str) -> &str {
    shape.strip_suffix(" critical").unwrap_or(shape)
}

#[derive(Default)]
struct Registry {
    format: u32,
    kinds: BTreeSet<String>,
    properties: BTreeMap<String, String>,
    old: BTreeMap<String, String>,
    defaults: BTreeMap<String, String>,
}

impl Registry {
    fn parse(text: &str) -> Self {
        let mut registry = Self::default();
        for line in text.lines() {
            if let Some(format) = line.strip_prefix("format ") {
                registry.format = format.trim().parse().unwrap_or(0);
            } else if let Some(kind) = line.strip_prefix("kind ") {
                registry.kinds.insert(kind.trim().to_owned());
            } else if let Some(property) = line.strip_prefix("property ") {
                if let Some((name, shape)) = property.split_once(": ") {
                    registry
                        .properties
                        .insert(name.to_owned(), shape.trim().to_owned());
                }
            } else if let Some(default) = line.strip_prefix("default ") {
                if let Some((path, value)) = default.split_once(" = ") {
                    registry
                        .defaults
                        .insert(path.to_owned(), value.trim().to_owned());
                }
            } else if let Some(old) = line.strip_prefix("old ")
                && let Some((from, to)) = old.split_once(" -> ")
            {
                let kind = from.rsplit_once('.').map_or("", |(kind, _)| kind);
                registry
                    .old
                    .insert(from.to_owned(), format!("{kind}.{}", to.trim()));
            }
        }
        registry
    }
}

pub fn freeze<R: crate::Model>(document: &crate::Document<R>) -> String {
    let bytes = document.to_bytes();
    let mut hex = String::new();
    for (index, byte) in bytes.iter().enumerate() {
        if index % 32 == 0 {
            hex.push_str("\n    ");
        }
        hex.push_str(&format!("{byte:02x}"));
    }
    format!(
        "pub const REGISTRY: &str = r#\"\n{}\"#;\n\npub const DOCUMENT: &str = \"{hex}\n\";\n",
        R::kind().describe()
    )
}

pub fn check_frozen<R: crate::Model>(registry: &str, document: &str) -> Result<(), Vec<String>> {
    compatible(registry.trim_start(), &R::kind().describe())?;
    let hex: String = document.split_whitespace().collect();
    let bytes: Option<Vec<u8>> = (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok())
        .collect();
    let bytes = bytes.ok_or_else(|| vec!["the frozen document is not hex".to_owned()])?;
    let loaded = crate::Document::<R>::from_bytes(&bytes)
        .map_err(|_| vec!["the frozen document no longer loads".to_owned()])?;
    let mut problems: Vec<String> = loaded
        .tree()
        .objects()
        .iter()
        .filter(|(_, object)| !object.kept().is_empty())
        .map(|(id, object)| {
            format!(
                "object {id} was not fully understood: kind {:?}, properties {:?}",
                object.kept().kind(),
                object.kept().properties().keys().collect::<Vec<_>>()
            )
        })
        .collect();
    let reloaded = crate::Document::<R>::from_bytes(&loaded.to_bytes())
        .map_err(|_| vec!["the frozen document does not load after saving".to_owned()])?;
    if reloaded != loaded {
        problems.push("the frozen document changes when it is saved and loaded again".to_owned());
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}
