use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::ExitCode;

use serde_json::{Map, Value, json};

use crate::starlark::render;

const LIBRARY_KINDS: [&str; 6] = ["lib", "rlib", "cdylib", "staticlib", "dylib", "proc-macro"];

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let (metadata, graphs) = arguments
        .split_first()
        .ok_or("usage: generate METADATA.json PLATFORM=UNIT_GRAPH.json...")?;
    let metadata = crate::json(metadata)?;
    let mut plans = Vec::new();
    for argument in graphs {
        let (name, path) = argument
            .split_once('=')
            .ok_or_else(|| format!("{argument} is not PLATFORM=PATH"))?;
        plans.push((name.to_owned(), crate::json(path)?));
    }
    print!("{}", generate(&metadata, &plans)?);
    Ok(ExitCode::SUCCESS)
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

fn kinds(target: &Value) -> BTreeSet<String> {
    target["kind"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|kind| kind.as_str().map(str::to_owned))
        .collect()
}

fn is_library(kinds: &BTreeSet<String>) -> bool {
    LIBRARY_KINDS.iter().any(|kind| kinds.contains(*kind))
}

fn relative(path: &str, base: &str) -> String {
    let path = Path::new(path);
    let base = Path::new(base);
    let mut common = 0;
    let path_parts: Vec<_> = path.components().collect();
    let base_parts: Vec<_> = base.components().collect();
    while common < path_parts.len()
        && common < base_parts.len()
        && path_parts[common] == base_parts[common]
    {
        common += 1;
    }
    let mut parts: Vec<String> = vec!["..".to_owned(); base_parts.len() - common];
    parts.extend(
        path_parts[common..]
            .iter()
            .map(|part| part.as_os_str().to_string_lossy().into_owned()),
    );
    if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    }
}

fn parent(path: &str) -> String {
    Path::new(path)
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn generate(metadata: &Value, plans: &[(String, Value)]) -> Result<String, String> {
    let root = text(metadata, "workspace_root");
    let members: BTreeMap<String, &Value> = metadata["packages"]
        .as_array()
        .ok_or("metadata has no packages")?
        .iter()
        .map(|package| (text(package, "id").to_owned(), package))
        .collect();
    let directory = |package: &Value| relative(&parent(text(package, "manifest_path")), root);
    let label = |package_id: &str| match members.get(package_id) {
        Some(package) => format!("//{}:{}", directory(package), text(package, "name")),
        None => {
            let name = package_id
                .split_once('#')
                .map(|(_, rest)| rest.split('@').next().unwrap_or(rest))
                .unwrap_or(package_id);
            format!("//third-party/rust:{name}")
        }
    };

    let mut crates: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    for (id, package) in &members {
        let here = parent(text(package, "manifest_path"));
        let mut library = Value::Null;
        let mut binaries = Vec::new();
        let mut examples = Vec::new();
        for target in package["targets"].as_array().into_iter().flatten() {
            let root_path = relative(text(target, "src_path"), &here);
            let kinds = kinds(target);
            let name = text(target, "name");
            if kinds.contains("example") {
                examples.push(json!({"crate_root": root_path, "name": name}));
            } else if is_library(&kinds) {
                library = json!({
                    "crate": name.replace('-', "_"),
                    "crate_root": root_path,
                    "proc_macro": kinds.contains("proc-macro"),
                });
            } else if kinds.contains("bin") {
                binaries.push(json!({"crate_root": root_path, "name": name}));
            }
        }
        binaries.sort_by(|a, b| text(a, "name").cmp(text(b, "name")));
        examples.sort_by(|a, b| text(a, "name").cmp(text(b, "name")));
        let mut entry = Map::new();
        entry.insert("binaries".into(), Value::Array(binaries));
        entry.insert("edition".into(), package["edition"].clone());
        entry.insert("examples".into(), Value::Array(examples));
        entry.insert("library".into(), library);
        entry.insert("name".into(), package["name"].clone());
        entry.insert("platforms".into(), Value::Object(Map::new()));
        entry.insert("version".into(), package["version"].clone());
        crates.insert(id.clone(), entry);
    }

    for (platform, graph) in plans {
        let units = graph["units"]
            .as_array()
            .ok_or("a unit graph has no units")?;
        let triple = units.iter().find_map(|unit| unit["platform"].as_str());
        let dependencies = |unit: &Value| -> BTreeSet<String> {
            unit["dependencies"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|dependency| units.get(dependency["index"].as_u64()? as usize))
                .filter(|other| !kinds(&other["target"]).contains("custom-build"))
                .filter(|other| other["pkg_id"] != unit["pkg_id"])
                .map(|other| label(text(other, "pkg_id")))
                .collect()
        };
        for unit in units {
            let Some(entry) = crates.get_mut(text(unit, "pkg_id")) else {
                continue;
            };
            let kinds = kinds(&unit["target"]);
            if !kinds.contains("proc-macro") && unit["platform"].as_str() != triple {
                continue;
            }
            let platforms = entry["platforms"]
                .as_object_mut()
                .expect("platforms is an object");
            let facts = platforms
                .entry(platform.clone())
                .or_insert_with(|| {
                    json!({"binaries": {}, "deps": [], "examples": {}, "features": [], "test_deps": [], "test_features": []})
                })
                .as_object_mut()
                .expect("a platform's facts are an object");
            let deps: Vec<String> = dependencies(unit).into_iter().collect();
            let mut features: Vec<String> = unit["features"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|feature| feature.as_str().map(str::to_owned))
                .collect();
            features.sort();
            let mode = text(unit, "mode");
            let name = text(&unit["target"], "name").to_owned();
            if is_library(&kinds) && mode == "build" {
                facts.insert("deps".into(), json!(deps));
                facts.insert("features".into(), json!(features));
            } else if is_library(&kinds) && mode == "test" {
                facts.insert("test_deps".into(), json!(deps));
                facts.insert("test_features".into(), json!(features));
            } else if kinds.contains("example") {
                facts["examples"][&name] = json!({"deps": deps, "features": features});
            } else if kinds.contains("bin") {
                facts["binaries"][&name] = json!(deps);
            }
        }
    }

    for entry in crates.values_mut() {
        for facts in entry["platforms"]
            .as_object_mut()
            .into_iter()
            .flat_map(|platforms| platforms.values_mut())
        {
            let strings = |value: &Value| -> BTreeSet<String> {
                value
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            };
            let deps = strings(&facts["deps"]);
            let dev_only: BTreeSet<String> = strings(&facts["test_deps"])
                .difference(&deps)
                .cloned()
                .collect();
            facts["test_deps"] = json!(dev_only);
            let binaries = facts["binaries"]
                .as_object_mut()
                .expect("binaries is an object");
            for deps in binaries.values_mut() {
                let kept: Vec<String> = strings(deps).difference(&dev_only).cloned().collect();
                *deps = json!(kept);
            }
        }
    }

    let by_directory: Map<String, Value> = crates
        .into_iter()
        .map(|(id, entry)| (directory(members[&id]), Value::Object(entry)))
        .collect();
    Ok(format!(
        "# @generated by ./scripts/buck from the workspace's Cargo.toml files.\n\
         # Do not edit by hand: crates/buck-tools writes it, and the macros in\n\
         # buck/cargo/defs.bzl read it.\n\ncrates = {}\n",
        render(&Value::Object(by_directory), 0)
    ))
}
