use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::ExitCode;

use serde_json::{Map, Value, json};

use crate::starlark::render;

const LIBRARY_KINDS: [&str; 6] = ["lib", "rlib", "cdylib", "staticlib", "dylib", "proc-macro"];

const HOST: &str = "linux-x86_64";

pub struct Inputs {
    pub metadata: Value,
    pub plans: Vec<(String, Value)>,
    pub lock: String,
    pub sizes: String,
    pub manifest: String,
}

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let usage =
        "usage: generate METADATA.json Cargo.lock SIZES Cargo.toml PLATFORM=UNIT_GRAPH.json...";
    let [metadata, lock, sizes, manifest, graphs @ ..] = arguments else {
        return Err(usage.to_owned());
    };
    let mut plans = Vec::new();
    for argument in graphs {
        let (name, path) = argument
            .split_once('=')
            .ok_or_else(|| format!("{argument} is not PLATFORM=PATH"))?;
        plans.push((name.to_owned(), crate::json(path)?));
    }
    let inputs = Inputs {
        metadata: crate::json(metadata)?,
        plans,
        lock: crate::read(lock)?,
        sizes: crate::read(sizes)?,
        manifest: crate::read(manifest)?,
    };
    print!("{}", generate(&inputs)?);
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

fn strings(value: &Value) -> BTreeSet<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
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

fn crate_key(package: &Value) -> String {
    format!("{}-{}", text(package, "name"), text(package, "version"))
}

fn checksums(lock: &str) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    let mut name = "";
    let mut version = "";
    for line in lock.lines().map(str::trim) {
        let value = |prefix: &str| {
            line.strip_prefix(prefix)
                .map(|rest| rest.trim().trim_matches('"'))
        };
        if line == "[[package]]" {
            name = "";
            version = "";
        } else if let Some(value) = value("name =") {
            name = value;
        } else if let Some(value) = value("version =") {
            version = value;
        } else if let Some(value) = value("checksum =") {
            found.insert(format!("{name}-{version}"), value.to_owned());
        }
    }
    found
}

fn sizes(listing: &str) -> BTreeMap<String, u64> {
    listing
        .lines()
        .filter_map(|line| {
            let (key, size) = line.split_once(' ')?;
            Some((key.to_owned(), size.trim().parse().ok()?))
        })
        .collect()
}

fn profile_flags(settings: &toml::Table) -> Vec<String> {
    let mut flags = Vec::new();
    match settings.get("opt-level") {
        Some(toml::Value::Integer(level)) => flags.push(format!("-Copt-level={level}")),
        Some(toml::Value::String(level)) => flags.push(format!("-Copt-level={level}")),
        _ => {}
    }
    match settings.get("debug") {
        Some(toml::Value::Boolean(false)) => flags.push("-Cdebuginfo=0".to_owned()),
        Some(toml::Value::Boolean(true)) => flags.push("-Cdebuginfo=2".to_owned()),
        Some(toml::Value::Integer(level)) => flags.push(format!("-Cdebuginfo={level}")),
        Some(toml::Value::String(level)) => flags.push(format!("-Cdebuginfo={level}")),
        _ => {}
    }
    for (key, flag) in [
        ("debug-assertions", "debug-assertions"),
        ("overflow-checks", "overflow-checks"),
    ] {
        if let Some(toml::Value::Boolean(on)) = settings.get(key) {
            flags.push(format!("-C{flag}={}", if *on { "on" } else { "off" }));
        }
    }
    flags
}

struct Profile {
    dependencies: Vec<String>,
    packages: BTreeMap<String, Vec<String>>,
}

impl Profile {
    fn parse(manifest: &str) -> Result<Self, String> {
        let manifest: toml::Table = manifest
            .parse()
            .map_err(|error| format!("Cargo.toml: {error}"))?;
        let overrides = manifest
            .get("profile")
            .and_then(|profile| profile.get("dev"))
            .and_then(|dev| dev.get("package"))
            .and_then(toml::Value::as_table);
        let mut profile = Profile {
            dependencies: Vec::new(),
            packages: BTreeMap::new(),
        };
        for (name, settings) in overrides.into_iter().flatten() {
            let Some(settings) = settings.as_table() else {
                continue;
            };
            let flags = profile_flags(settings);
            if name == "*" {
                profile.dependencies = flags;
            } else {
                profile.packages.insert(name.clone(), flags);
            }
        }
        Ok(profile)
    }

    fn flags(&self, name: &str, member: bool) -> Vec<String> {
        let mut flags = if member {
            Vec::new()
        } else {
            self.dependencies.clone()
        };
        flags.extend(self.packages.get(name).into_iter().flatten().cloned());
        flags
    }
}

#[derive(Default)]
struct Facts {
    deps: BTreeSet<String>,
    features: BTreeSet<String>,
    named_deps: BTreeMap<String, String>,
}

impl Facts {
    fn add(&mut self, features: &Value, dependencies: Vec<Dependency>) {
        self.features.extend(strings(features));
        for dependency in dependencies {
            if dependency.extern_name == dependency.crate_name {
                self.deps.insert(dependency.label);
            } else {
                self.named_deps
                    .insert(dependency.extern_name, dependency.label);
            }
        }
    }

    fn finish(&self) -> Value {
        let named: BTreeSet<&String> = self.named_deps.values().collect();
        let deps: Vec<&String> = self
            .deps
            .iter()
            .filter(|label| !named.contains(label))
            .collect();
        let mut facts = Map::new();
        if !deps.is_empty() {
            facts.insert("deps".into(), json!(deps));
        }
        if !self.features.is_empty() {
            facts.insert("features".into(), json!(self.features));
        }
        if !self.named_deps.is_empty() {
            facts.insert("named_deps".into(), json!(self.named_deps));
        }
        Value::Object(facts)
    }
}

struct Dependency {
    label: String,
    extern_name: String,
    crate_name: String,
}

#[derive(Default)]
struct ThirdParty {
    build_script: Facts,
    platforms: BTreeMap<String, Facts>,
}

pub fn generate(inputs: &Inputs) -> Result<String, String> {
    let metadata = &inputs.metadata;
    let root = text(metadata, "workspace_root");
    let workspace_ids: BTreeSet<&str> = metadata["workspace_members"]
        .as_array()
        .ok_or("metadata has no workspace_members")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let packages: BTreeMap<String, &Value> = metadata["packages"]
        .as_array()
        .ok_or("metadata has no packages")?
        .iter()
        .map(|package| (text(package, "id").to_owned(), package))
        .collect();
    let members: BTreeMap<&String, &&Value> = packages
        .iter()
        .filter(|(id, _)| workspace_ids.contains(id.as_str()))
        .collect();
    let directory = |package: &Value| relative(&parent(text(package, "manifest_path")), root);
    let label = |package_id: &str, local: bool| -> String {
        let package = packages[package_id];
        if workspace_ids.contains(package_id) {
            format!("//{}:{}", directory(package), text(package, "name"))
        } else if local {
            format!(":{}", crate_key(package))
        } else {
            format!("//third-party/rust:{}", crate_key(package))
        }
    };
    let profile = Profile::parse(&inputs.manifest)?;

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
        entry.insert(
            "profile_flags".into(),
            json!(profile.flags(text(package, "name"), true)),
        );
        entry.insert("version".into(), package["version"].clone());
        crates.insert((*id).clone(), entry);
    }

    let mut third_party: BTreeMap<String, ThirdParty> = BTreeMap::new();

    for (platform, graph) in &inputs.plans {
        let units = graph["units"]
            .as_array()
            .ok_or("a unit graph has no units")?;
        let triple = units.iter().find_map(|unit| unit["platform"].as_str());
        let dependencies = |unit: &Value, local: bool| -> Vec<Dependency> {
            unit["dependencies"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|dependency| {
                    let other = units.get(dependency["index"].as_u64()? as usize)?;
                    if kinds(&other["target"]).contains("custom-build")
                        || other["pkg_id"] == unit["pkg_id"]
                    {
                        return None;
                    }
                    Some(Dependency {
                        label: label(text(other, "pkg_id"), local),
                        extern_name: text(dependency, "extern_crate_name").to_owned(),
                        crate_name: text(&other["target"], "name").replace('-', "_"),
                    })
                })
                .collect()
        };
        for unit in units {
            let id = text(unit, "pkg_id");
            let kinds = kinds(&unit["target"]);
            let mode = text(unit, "mode");
            if !workspace_ids.contains(id) {
                if !packages.contains_key(id) {
                    return Err(format!("{id} is in a plan but not in the metadata"));
                }
                let destination = match unit["platform"].as_str() {
                    None => HOST.to_owned(),
                    Some(_) => platform.clone(),
                };
                let entry = third_party.entry(id.to_owned()).or_default();
                if kinds.contains("custom-build") && mode == "build" {
                    entry
                        .build_script
                        .add(&Value::Null, dependencies(unit, true));
                } else if is_library(&kinds) && mode == "build" {
                    entry
                        .platforms
                        .entry(destination)
                        .or_default()
                        .add(&unit["features"], dependencies(unit, true));
                }
                continue;
            }
            let Some(entry) = crates.get_mut(id) else {
                continue;
            };
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
            let deps: BTreeSet<String> = dependencies(unit, false)
                .into_iter()
                .map(|dependency| dependency.label)
                .collect();
            let features = strings(&unit["features"]);
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

    let checksums = checksums(&inputs.lock);
    let sizes = sizes(&inputs.sizes);
    let mut rendered_third_party = Map::new();
    for (id, facts) in &third_party {
        let package = packages[id];
        let key = crate_key(package);
        let here = parent(text(package, "manifest_path"));
        let mut library = Value::Null;
        let mut build_script = Value::Null;
        for target in package["targets"].as_array().into_iter().flatten() {
            let kinds = kinds(target);
            let entry = json!({
                "crate_root": relative(text(target, "src_path"), &here),
                "edition": target["edition"],
            });
            if kinds.contains("custom-build") {
                build_script = entry;
            } else if is_library(&kinds) {
                let mut entry = entry;
                entry["crate"] = json!(text(target, "name").replace('-', "_"));
                entry["proc_macro"] = json!(kinds.contains("proc-macro"));
                library = entry;
            }
        }
        if library.is_null() {
            continue;
        }
        if !build_script.is_null() {
            let script = facts.build_script.finish();
            for field in ["deps", "named_deps"] {
                if let Some(value) = script.get(field) {
                    build_script[field] = value.clone();
                }
            }
        }
        let sha256 = checksums
            .get(&key)
            .ok_or_else(|| format!("Cargo.lock has no checksum for {key}"))?;
        let size = sizes
            .get(&key)
            .ok_or_else(|| format!("cargo fetch did not download {key}"))?;
        let mut entry = Map::new();
        entry.insert("build_script".into(), build_script);
        entry.insert("library".into(), library);
        entry.insert("name".into(), package["name"].clone());
        entry.insert(
            "platforms".into(),
            Value::Object(
                facts
                    .platforms
                    .iter()
                    .map(|(platform, facts)| (platform.clone(), facts.finish()))
                    .collect(),
            ),
        );
        let flags = profile.flags(text(package, "name"), false);
        if !flags.is_empty() {
            entry.insert("profile_flags".into(), json!(flags));
        }
        entry.insert("sha256".into(), json!(sha256));
        entry.insert("size_bytes".into(), json!(size));
        entry.insert("version".into(), package["version"].clone());
        if let Some(links) = package["links"].as_str() {
            entry.insert("links".into(), json!(links));
        }
        let authors: Vec<&str> = package["authors"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let mut env = Map::new();
        for (name, value) in [
            ("CARGO_PKG_AUTHORS", authors.join(":")),
            (
                "CARGO_PKG_DESCRIPTION",
                text(package, "description").to_owned(),
            ),
            ("CARGO_PKG_HOMEPAGE", text(package, "homepage").to_owned()),
            ("CARGO_PKG_LICENSE", text(package, "license").to_owned()),
            ("CARGO_PKG_README", text(package, "readme").to_owned()),
            (
                "CARGO_PKG_REPOSITORY",
                text(package, "repository").to_owned(),
            ),
            (
                "CARGO_PKG_RUST_VERSION",
                text(package, "rust_version").to_owned(),
            ),
        ] {
            if !value.is_empty() {
                env.insert(name.into(), json!(value));
            }
        }
        if !env.is_empty() {
            entry.insert("env".into(), Value::Object(env));
        }
        rendered_third_party.insert(key, Value::Object(entry));
    }

    let by_directory: Map<String, Value> = crates
        .into_iter()
        .map(|(id, entry)| (directory(packages[&id]), Value::Object(entry)))
        .collect();
    Ok(format!(
        "# @generated by ./scripts/buck from the workspace's Cargo.toml files.\n\
         # Do not edit by hand: crates/buck-tools writes it from cargo's own plans,\n\
         # and buck/cargo/defs.bzl and buck/cargo/third_party.bzl read it.\n\n\
         crates = {}\n\nthird_party = {}\n",
        render(&Value::Object(by_directory), 0),
        render(&Value::Object(rendered_third_party), 0)
    ))
}
