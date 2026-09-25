use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

#[derive(Default)]
struct Options {
    out: String,
    values: BTreeMap<String, String>,
    java: Vec<String>,
    libraries: Vec<String>,
}

impl Options {
    fn get(&self, key: &str) -> Result<&str, String> {
        self.values
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| format!("--{key} is required"))
    }
}

fn parse(arguments: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut rest = arguments.iter();
    while let Some(argument) = rest.next() {
        let Some(key) = argument.strip_prefix("--") else {
            options.out = argument.clone();
            continue;
        };
        let (key, value) = match key.split_once('=') {
            Some((key, value)) => (key.to_owned(), value.to_owned()),
            None => (
                key.to_owned(),
                rest.next().ok_or(format!("--{key} needs a value"))?.clone(),
            ),
        };
        match key.as_str() {
            "java" => options.java.push(value),
            "library" => options.libraries.push(value),
            _ => {
                options.values.insert(key, value);
            }
        }
    }
    if options.out.is_empty() {
        return Err(
            "usage: apk OUT --jdk DIR --build-tools DIR --platform DIR --manifest FILE ...".into(),
        );
    }
    Ok(options)
}

fn entry<W: Write + std::io::Seek>(
    archive: &mut ZipWriter<W>,
    name: &str,
    data: &[u8],
    stored: bool,
) -> Result<(), String> {
    let method = if stored {
        CompressionMethod::Stored
    } else {
        CompressionMethod::Deflated
    };
    let options = SimpleFileOptions::default()
        .compression_method(method)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644);
    archive
        .start_file(name, options)
        .map_err(|error| error.to_string())?;
    archive.write_all(data).map_err(|error| error.to_string())
}

fn sorted_files(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut found = Vec::new();
    files(directory, &mut found)?;
    found.sort();
    Ok(found)
}

fn unzip(archive: &Path, into: &Path) -> Result<(), String> {
    let file =
        std::fs::File::open(archive).map_err(|error| format!("{}: {error}", archive.display()))?;
    ZipArchive::new(file)
        .and_then(|mut zip| zip.extract(into))
        .map_err(|error| format!("{}: {error}", archive.display()))
}

fn manifest_package(manifest: &Path) -> Result<String, String> {
    let text = crate::read(&manifest.to_string_lossy())?;
    let start = text
        .find("package=\"")
        .ok_or_else(|| format!("{} names no package", manifest.display()))?
        + "package=\"".len();
    let end = text[start..]
        .find('"')
        .ok_or_else(|| format!("{} has an unterminated package", manifest.display()))?;
    Ok(text[start..start + end].to_owned())
}

struct JavaLibraries {
    jars: Vec<PathBuf>,
    resources: Vec<PathBuf>,
    packages: Vec<String>,
}

fn java_libraries(
    directory: &Path,
    build_tools: &Path,
    scratch: &Path,
) -> Result<JavaLibraries, String> {
    let mut libraries = JavaLibraries {
        jars: Vec::new(),
        resources: Vec::new(),
        packages: Vec::new(),
    };
    for (index, path) in sorted_files(directory)?.into_iter().enumerate() {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("jar") => libraries.jars.push(path),
            Some("aar") => {
                let unpacked = scratch.join(format!("aar/{index}"));
                unzip(&path, &unpacked)?;
                let classes = unpacked.join("classes.jar");
                if classes.exists() {
                    libraries.jars.push(classes);
                }
                let bundled = unpacked.join("libs");
                if bundled.is_dir() {
                    libraries.jars.extend(
                        sorted_files(&bundled)?.into_iter().filter(|jar| {
                            jar.extension().is_some_and(|extension| extension == "jar")
                        }),
                    );
                }
                libraries
                    .packages
                    .push(manifest_package(&unpacked.join("AndroidManifest.xml"))?);
                let resources = unpacked.join("res");
                if resources.is_dir() {
                    let compiled = scratch.join(format!("res/{index}.zip"));
                    compile_resources(build_tools, &resources, &compiled)?;
                    libraries.resources.push(compiled);
                }
            }
            _ => return Err(format!("{}: not a .jar or .aar", path.display())),
        }
    }
    libraries.packages.sort();
    libraries.packages.dedup();
    Ok(libraries)
}

fn compile_resources(build_tools: &Path, resources: &Path, out: &Path) -> Result<(), String> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    crate::status(
        Command::new(build_tools.join("aapt2"))
            .arg("compile")
            .arg("--dir")
            .arg(resources)
            .arg("-o")
            .arg(out),
    )
}

fn files(directory: &Path, found: &mut Vec<PathBuf>) -> Result<(), String> {
    for item in
        std::fs::read_dir(directory).map_err(|error| format!("{}: {error}", directory.display()))?
    {
        let path = item.map_err(|error| error.to_string())?.path();
        if path.is_dir() {
            files(&path, found)?;
        } else {
            found.push(path);
        }
    }
    Ok(())
}

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let options = parse(arguments)?;
    let build_tools = Path::new(options.get("build-tools")?);
    let jdk = Path::new(options.get("jdk")?);
    let android_jar = Path::new(options.get("platform")?).join("android.jar");
    let scratch = std::env::temp_dir().join(format!("apk-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).map_err(|error| error.to_string())?;

    let manifest = crate::read(options.get("manifest")?)?
        .replace("${be3Label}", options.get("label")?)
        .replacen(
            "<manifest ",
            &format!("<manifest package=\"{}\" ", options.get("package")?),
            1,
        )
        .replacen(
            "<application ",
            "<application android:extractNativeLibs=\"false\" ",
            1,
        );
    let manifest_path = scratch.join("AndroidManifest.xml");
    std::fs::write(&manifest_path, manifest).map_err(|error| error.to_string())?;

    let libraries = match options.get("java-libraries") {
        Ok(directory) => java_libraries(Path::new(directory), build_tools, &scratch)?,
        Err(_) => JavaLibraries {
            jars: Vec::new(),
            resources: Vec::new(),
            packages: Vec::new(),
        },
    };
    let mut resources = libraries.resources.clone();
    if let Ok(directory) = options.get("resources") {
        let compiled = scratch.join("res/app.zip");
        compile_resources(build_tools, Path::new(directory), &compiled)?;
        resources.push(compiled);
    }

    let linked = scratch.join("linked.apk");
    let generated = scratch.join("generated");
    let mut link = Command::new(build_tools.join("aapt2"));
    link.arg("link");
    for compiled in &resources {
        link.arg("-R").arg(compiled);
    }
    if !resources.is_empty() {
        link.arg("--auto-add-overlay");
    }
    if !options.java.is_empty() {
        link.arg("--java").arg(&generated);
    }
    if !libraries.packages.is_empty() {
        link.arg("--extra-packages")
            .arg(libraries.packages.join(":"));
    }
    crate::status(
        link.arg("--manifest")
            .arg(&manifest_path)
            .arg("-I")
            .arg(&android_jar)
            .args(["--min-sdk-version", options.get("min-sdk")?])
            .args(["--target-sdk-version", options.get("target-sdk")?])
            .args(["--version-code", options.get("version-code")?])
            .args(["--version-name", options.get("version-name")?])
            .args(["--rename-manifest-package", options.get("application-id")?])
            .arg("--debug-mode")
            .arg("-o")
            .arg(&linked),
    )?;

    let mut dexes = Vec::new();
    if !options.java.is_empty() {
        let mut sources: Vec<PathBuf> = options.java.iter().map(PathBuf::from).collect();
        if generated.is_dir() {
            sources.extend(sorted_files(&generated)?);
        }
        let classpath = std::env::join_paths(
            std::iter::once(android_jar.clone()).chain(libraries.jars.iter().cloned()),
        )
        .map_err(|error| error.to_string())?;
        let classes = scratch.join("classes");
        crate::status(
            Command::new(jdk.join("bin/javac"))
                .args([
                    "-nowarn",
                    "-Xlint:-options",
                    "-source",
                    "11",
                    "-target",
                    "11",
                    "-classpath",
                ])
                .arg(&classpath)
                .arg("-d")
                .arg(&classes)
                .args(&sources),
        )?;
        let mut compiled = Vec::new();
        files(&classes, &mut compiled)?;
        compiled.retain(|path| {
            path.extension()
                .is_some_and(|extension| extension == "class")
        });
        compiled.sort();
        let dexed = scratch.join("dex");
        std::fs::create_dir_all(&dexed).map_err(|error| error.to_string())?;
        crate::status(
            Command::new(jdk.join("bin/java"))
                .arg("-cp")
                .arg(build_tools.join("lib/d8.jar"))
                .args([
                    "com.android.tools.r8.D8",
                    "--debug",
                    "--min-api",
                    options.get("min-sdk")?,
                    "--lib",
                ])
                .arg(&android_jar)
                .arg("--output")
                .arg(&dexed)
                .args(&compiled)
                .args(&libraries.jars),
        )?;
        dexes = sorted_files(&dexed)?;
    }

    let unaligned = scratch.join("unaligned.apk");
    let mut archive =
        ZipWriter::new(std::fs::File::create(&unaligned).map_err(|error| error.to_string())?);
    let mut source =
        ZipArchive::new(std::fs::File::open(&linked).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let mut names: Vec<String> = source.file_names().map(str::to_owned).collect();
    names.sort();
    for name in names {
        let mut file = source.by_name(&name).map_err(|error| error.to_string())?;
        let stored = file.compression() == CompressionMethod::Stored;
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|error| error.to_string())?;
        entry(&mut archive, &name, &data, stored)?;
    }
    for dex in dexes {
        let name = dex
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let data = std::fs::read(&dex).map_err(|error| error.to_string())?;
        entry(&mut archive, &name, &data, false)?;
    }
    let mut libraries: Vec<PathBuf> = options.libraries.iter().map(PathBuf::from).collect();
    libraries.sort_by_key(|library| library.file_name().map(|name| name.to_owned()));
    for library in libraries {
        let name = library
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let data =
            std::fs::read(&library).map_err(|error| format!("{}: {error}", library.display()))?;
        entry(&mut archive, &format!("lib/arm64-v8a/{name}"), &data, true)?;
    }
    if let Ok(assets) = options.get("assets") {
        let assets = Path::new(assets);
        let mut found = Vec::new();
        files(assets, &mut found)?;
        let mut named: Vec<(String, PathBuf)> = found
            .into_iter()
            .map(|path| {
                let name = path
                    .strip_prefix(assets)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                (name, path)
            })
            .collect();
        named.sort();
        for (name, path) in named {
            let data = std::fs::read(&path).map_err(|error| error.to_string())?;
            entry(&mut archive, &format!("assets/{name}"), &data, false)?;
        }
    }
    archive.finish().map_err(|error| error.to_string())?;

    crate::status(
        Command::new(build_tools.join("zipalign"))
            .args(["-P", "16", "-f", "4"])
            .arg(&unaligned)
            .arg(&options.out),
    )?;
    let _ = std::fs::remove_dir_all(&scratch);
    Ok(ExitCode::SUCCESS)
}
