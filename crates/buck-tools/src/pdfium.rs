use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde_json::{Map, Value, json};

use crate::starlark::{quote, render};

const PDFIUM: &str = "https://pdfium.googlesource.com/pdfium";
const ROOT_TARGET: &str = "//:pdfium";
const OUTPUT: &str = "third-party/pdfium/sources.bzl";
const CHECKOUT: &str = "target/pdfium";

const READ_BY_GN: [&str; 12] = [
    "build",
    "buildtools",
    "base/allocator/partition_allocator",
    "third_party/abseil-cpp",
    "third_party/dragonbox/src",
    "third_party/fast_float/src",
    "third_party/freetype/src",
    "third_party/harfbuzz/src",
    "third_party/icu",
    "third_party/libjpeg_turbo",
    "third_party/simdutf",
    "third_party/zlib",
];

const HEADERS_ONLY: [&str; 1] = ["build"];

const GN_ARGS: [&str; 26] = [
    "target_os=\"linux\"",
    "target_cpu=\"riscv64\"",
    "is_debug=false",
    "is_component_build=false",
    "is_clang=true",
    "clang_base_path=\"/usr\"",
    "clang_use_chrome_plugins=false",
    "use_custom_libcxx=false",
    "use_sysroot=false",
    "use_remoteexec=false",
    "use_siso=false",
    "treat_warnings_as_errors=false",
    "enable_rust=false",
    "pdf_is_standalone=true",
    "pdf_enable_xfa=false",
    "pdf_enable_v8=false",
    "pdf_use_skia=false",
    "pdf_use_partition_alloc=false",
    "pdf_enable_fontations=false",
    "pdf_enable_rust_bmp=false",
    "pdf_enable_rust_jpeg=false",
    "pdf_enable_rust_png=false",
    "use_system_libpng=true",
    "pdf_bundle_freetype=true",
    "use_system_lcms2=false",
    "use_system_libopenjpeg2=false",
];

const GCLIENT_ARGS: &str = "android_ndk_version = \"\"\n\
                            build_with_chromium = false\n\
                            checkout_android = false\n\
                            checkout_libpng = false\n\
                            checkout_skia = false\n";

const DROPPED_DEFINES: [&str; 4] = ["USE_UDEV", "USE_AURA=1", "USE_GLIB=1", "USE_OZONE=1"];

const COMPILED: [&str; 3] = [".c", ".cc", ".cpp"];

const DEPS_READER: &str = r#"
import json, sys
scope = {"Var": lambda name: "{" + name + "}", "Str": str}
exec(open(sys.argv[1]).read(), scope)
variables = scope["vars"]
found = {}
for path, dep in scope["deps"].items():
    if isinstance(dep, dict):
        if dep.get("dep_type", "git") != "git" or "url" not in dep:
            continue
        dep = dep["url"]
    url = dep.format(**variables)
    if "@" in url:
        repository, commit = url.rsplit("@", 1)
        found[path] = [repository, commit]
print(json.dumps({"deps": found, "gn_version": variables["gn_version"]}))
"#;

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let [revision] = arguments else {
        return Err(
            "usage: pdfium REVISION (a PDFium commit, or a branch such as chromium/8046), \
                    from inside the repository"
                .to_owned(),
        );
    };
    let root = repository()?;
    let checkout = root.join(CHECKOUT);
    std::fs::create_dir_all(&checkout).map_err(|error| format!("{CHECKOUT}: {error}"))?;
    let commit = fetch(&checkout, PDFIUM, revision)?;
    let deps = dependencies(&checkout)?;
    let mut repositories = Map::new();
    repositories.insert(String::new(), json!({"url": PDFIUM, "commit": commit}));
    std::thread::scope(|scope| {
        let fetches: Vec<_> = READ_BY_GN
            .iter()
            .map(|path| {
                let checkout = &checkout;
                let deps = &deps;
                scope.spawn(move || {
                    let [url, commit] = pinned(deps, path)?;
                    fetch(&checkout.join(path), url, commit)?;
                    Ok::<_, String>((path.to_string(), json!({"url": url, "commit": commit})))
                })
            })
            .collect();
        for fetched in fetches {
            let (path, repository) = fetched.join().map_err(|_| "a fetch panicked")??;
            repositories.insert(path, repository);
        }
        Ok::<_, String>(())
    })?;
    write(
        &checkout.join("build/config/gclient_args.gni"),
        GCLIENT_ARGS,
    )?;
    let gn = gn(
        &checkout,
        deps["gn_version"]
            .as_str()
            .ok_or("DEPS names no gn_version")?,
    )?;
    crate::status(
        Command::new(gn)
            .current_dir(&checkout)
            .arg("gen")
            .arg("out")
            .arg(format!("--root-pattern={ROOT_TARGET}"))
            .arg("--script-executable=python3")
            .arg("--ide=json")
            .arg(format!("--args={}", GN_ARGS.join(" "))),
    )?;
    let project = crate::json(&checkout.join("out/project.json").display().to_string())?;
    let libraries = libraries(&project)?;
    repositories.retain(|path, _| path.is_empty() || used(&libraries, path));
    write(
        &root.join(OUTPUT),
        &sources(revision, &repositories, &libraries),
    )?;
    println!(
        "{OUTPUT}: PDFium {commit}, {} libraries from {} repositories",
        libraries.len(),
        repositories.len()
    );
    Ok(ExitCode::SUCCESS)
}

pub fn libraries(project: &Value) -> Result<Map<String, Value>, String> {
    let targets = project["targets"]
        .as_object()
        .ok_or("project.json has no targets")?;
    let mut order = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![ROOT_TARGET.to_owned()];
    while let Some(label) = stack.pop() {
        if !seen.insert(label.clone()) {
            continue;
        }
        let target = targets
            .get(&label)
            .ok_or_else(|| format!("project.json has no {label}"))?;
        for dep in strings(&target["deps"]).into_iter().rev() {
            stack.push(dep);
        }
        order.push(label);
    }
    let mut libraries = Map::new();
    for label in order {
        let target = &targets[&label];
        let srcs: Vec<String> = strings(&target["sources"])
            .into_iter()
            .filter(|source| COMPILED.iter().any(|extension| source.ends_with(extension)))
            .map(|source| relative(&source))
            .collect();
        if srcs.is_empty() {
            continue;
        }
        let defines: Vec<String> = strings(&target["defines"])
            .into_iter()
            .filter(|define| !DROPPED_DEFINES.contains(&define.as_str()))
            .collect();
        let include_dirs: Vec<String> = strings(&target["include_dirs"])
            .into_iter()
            .map(|directory| relative(&directory).trim_end_matches('/').to_owned())
            .filter(|directory| directory != "out" && !directory.starts_with("out/"))
            .collect();
        let c_flags: Vec<String> = strings(&target["cflags_c"])
            .into_iter()
            .filter(|flag| flag.starts_with("-std="))
            .collect();
        let cxx_flags: Vec<String> = strings(&target["cflags_cc"])
            .into_iter()
            .filter(|flag| flag.starts_with("-std=") || flag == "-frtti")
            .collect();
        libraries.insert(
            name(&label),
            json!({
                "label": label,
                "srcs": srcs,
                "defines": defines,
                "include_dirs": include_dirs,
                "c_flags": c_flags,
                "cxx_flags": cxx_flags,
            }),
        );
    }
    Ok(libraries)
}

pub fn used(libraries: &Map<String, Value>, path: &str) -> bool {
    if HEADERS_ONLY.contains(&path) {
        return true;
    }
    let prefix = format!("{path}/");
    libraries.values().any(|library| {
        ["srcs", "include_dirs"].iter().any(|key| {
            strings(&library[*key])
                .iter()
                .any(|file| file == path || file.starts_with(&prefix))
        })
    })
}

pub fn sources(
    revision: &str,
    repositories: &Map<String, Value>,
    libraries: &Map<String, Value>,
) -> String {
    format!(
        "# @generated by ./scripts/buck run //:pdfium -- {revision}. Do not edit by hand:\n\
         # crates/buck-tools writes it from PDFium's GN build, and third-party/pdfium/BUCK\n\
         # reads it.\n\n\
         revision = {}\n\n\
         repositories = {}\n\n\
         libraries = {}\n",
        quote(revision),
        render(&Value::Object(repositories.clone()), 0),
        render(&Value::Object(libraries.clone()), 0)
    )
}

fn name(label: &str) -> String {
    relative(label).replace(['/', ':'], "_")
}

fn relative(path: &str) -> String {
    path.trim_start_matches("//").to_owned()
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn pinned<'a>(deps: &'a Value, path: &str) -> Result<[&'a str; 2], String> {
    let pin = &deps["deps"][path];
    match (pin[0].as_str(), pin[1].as_str()) {
        (Some(url), Some(commit)) => Ok([url, commit]),
        _ => Err(format!("DEPS pins no git repository at {path}")),
    }
}

fn repository() -> Result<PathBuf, String> {
    let here = std::env::current_dir().map_err(|error| error.to_string())?;
    here.ancestors()
        .find(|directory| {
            directory.join(".buckconfig").is_file() && directory.join("third-party/pdfium").is_dir()
        })
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("{} is not inside the repository", here.display()))
}

fn git(directory: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(directory);
    command
}

fn head(directory: &Path) -> Option<String> {
    let output = git(directory).args(["rev-parse", "HEAD"]).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn fetch(directory: &Path, url: &str, revision: &str) -> Result<String, String> {
    if head(directory).as_deref() == Some(revision) {
        return Ok(revision.to_owned());
    }
    std::fs::create_dir_all(directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?;
    if !directory.join(".git").exists() {
        crate::status(git(directory).args(["init", "--quiet"]))?;
    }
    eprintln!("Fetching {url} at {revision}...");
    crate::status(git(directory).args(["fetch", "--quiet", "--depth", "1", url, revision]))?;
    crate::status(git(directory).args(["checkout", "--quiet", "--force", "FETCH_HEAD"]))?;
    head(directory).ok_or_else(|| format!("{} has no HEAD", directory.display()))
}

fn dependencies(checkout: &Path) -> Result<Value, String> {
    let output = Command::new("python3")
        .arg("-c")
        .arg(DEPS_READER)
        .arg(checkout.join("DEPS"))
        .output()
        .map_err(|error| format!("python3: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "reading PDFium's DEPS failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| format!("DEPS: {error}"))
}

fn gn(checkout: &Path, version: &str) -> Result<PathBuf, String> {
    let platform = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "linux-amd64",
        ("linux", "aarch64") => "linux-arm64",
        ("macos", "x86_64") => "mac-amd64",
        ("macos", "aarch64") => "mac-arm64",
        (os, arch) => return Err(format!("no gn is published for {os} on {arch}")),
    };
    let directory = checkout.join("out/gn");
    let gn = directory.join("gn");
    let stamp = directory.join("version");
    if gn.is_file() && std::fs::read_to_string(&stamp).ok().as_deref() == Some(version) {
        return Ok(gn);
    }
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?;
    let archive = directory.join("gn.zip");
    crate::status(
        Command::new("curl")
            .args([
                "--fail",
                "--location",
                "--silent",
                "--show-error",
                "--proto",
                "=https",
            ])
            .arg("--output")
            .arg(&archive)
            .arg(format!(
                "https://chrome-infra-packages.appspot.com/dl/gn/gn/{platform}/+/{version}"
            )),
    )?;
    let file =
        std::fs::File::open(&archive).map_err(|error| format!("{}: {error}", archive.display()))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|error| format!("{}: {error}", archive.display()))?;
    let mut entry = zip
        .by_name("gn")
        .map_err(|error| format!("{}: {error}", archive.display()))?;
    let mut binary = Vec::new();
    std::io::Read::read_to_end(&mut entry, &mut binary).map_err(|error| error.to_string())?;
    std::fs::write(&gn, binary).map_err(|error| format!("{}: {error}", gn.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&gn, std::fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("{}: {error}", gn.display()))?;
    }
    write(&stamp, version)?;
    Ok(gn)
}

fn write(path: &Path, contents: &str) -> Result<(), String> {
    std::fs::write(path, contents).map_err(|error| format!("{}: {error}", path.display()))
}
