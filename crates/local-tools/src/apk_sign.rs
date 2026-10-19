use std::{
    fs,
    io::{IsTerminal, Write},
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    sync::atomic::{AtomicU64, Ordering},
};

use base64::Engine as _;
use wasmtime::{Cache, Config, Engine, Linker, Module, Store};
use wasmtime_wasi::{FsPerms, WasiCtxBuilder, p1};

const USAGE: &str = "usage: apk-sign MODULE --in APK --out APK [--keystore FILE] [--password-file FILE] [--build-info JSON] [--signing dev|ci|release] [--install --activity PACKAGE/ACTIVITY]";

struct Scratch(PathBuf);

impl Scratch {
    fn new(parent: &Path) -> Result<Self, String> {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let parent = fs::canonicalize(parent).map_err(|error| error.to_string())?;
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = parent.join(format!(
            ".apk-sign-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        builder
            .create(&directory)
            .map_err(|error| error.to_string())?;
        Ok(Self(directory))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn main(arguments: Vec<String>) -> ExitCode {
    match run(arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("apk-sign: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<String>) -> Result<(), String> {
    let mut arguments = arguments.into_iter();
    let module = PathBuf::from(arguments.next().ok_or(USAGE)?);
    let (mut input, mut output, mut key, mut password, mut metadata, mut activity) =
        (None, None, None, None, None, None);
    let mut signing = "dev".to_owned();
    let mut install = false;
    while let Some(argument) = arguments.next() {
        if argument == "--install" {
            install = true;
            continue;
        }
        let value = arguments.next().ok_or(USAGE)?;
        match argument.as_str() {
            "--in" => input = Some(PathBuf::from(value)),
            "--out" => output = Some(PathBuf::from(value)),
            "--keystore" => key = Some(PathBuf::from(value)),
            "--password-file" => password = Some(PathBuf::from(value)),
            "--build-info" => metadata = Some(PathBuf::from(value)),
            "--signing" => signing = value,
            "--activity" => activity = Some(value),
            _ => return Err(USAGE.into()),
        }
    }
    let input = input.ok_or(USAGE)?;
    let output = output.ok_or(USAGE)?;
    if install && activity.is_none() {
        return Err("--install needs --activity PACKAGE/ACTIVITY".into());
    }
    let key = match key {
        Some(key) => key,
        None => match signing.as_str() {
            "dev" => {
                let key = PathBuf::from("target/android-debug.keystore");
                if !key.exists() {
                    generate_key(&module, &key, 2048)?;
                }
                key
            }
            "ci" => ci_key(),
            "release" => release_key(&module)?,
            _ => return Err(USAGE.into()),
        },
    };
    let password = match password {
        Some(path) => fs::read(path).map_err(|error| error.to_string())?,
        None => b"android".to_vec(),
    };
    sign(
        &module,
        &input,
        &output,
        &key,
        metadata.as_deref(),
        &password,
    )?;
    if install {
        let adb = adb();
        status(Command::new(&adb).args(["install", "-r"]).arg(&output))?;
        status(Command::new(&adb).args([
            "shell",
            "am",
            "start",
            "-n",
            activity.as_deref().unwrap(),
        ]))?;
    }
    Ok(())
}

fn status(command: &mut Command) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("{command:?}: {error}"))?;
    if !status.success() {
        return Err(format!("{command:?}: {status}"));
    }
    Ok(())
}

fn adb() -> PathBuf {
    if let Some(path) = std::env::var_os("ADB") {
        return path.into();
    }
    let name = if cfg!(windows) { "adb.exe" } else { "adb" };
    let mut roots: Vec<PathBuf> = ["ANDROID_HOME", "ANDROID_SDK_ROOT"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .collect();
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        let home = PathBuf::from(home);
        roots.extend([
            home.join("Android/Sdk"),
            home.join("Library/Android/sdk"),
            home.join("AppData/Local/Android/Sdk"),
        ]);
    }
    roots
        .into_iter()
        .map(|root| root.join("platform-tools").join(name))
        .find(|path| path.is_file())
        .unwrap_or_else(|| name.into())
}

pub(crate) fn ci_key() -> PathBuf {
    std::env::var_os("BE3_ANDROID_CI_KEYSTORE")
        .map(PathBuf::from)
        .unwrap_or_else(|| "buck/android/ci-keystore.base64".into())
}

pub(crate) fn release_key(module: &Path) -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("BE3_ANDROID_RELEASE_KEYSTORE") {
        return Ok(path.into());
    }
    let config = if let Some(config) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(config)
    } else {
        PathBuf::from(
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .ok_or("HOME or USERPROFILE is required")?,
        )
        .join(".config")
    };
    let saved = config.join("be3/android-release-keystore.base64");
    if saved.is_file() {
        return Ok(saved);
    }
    if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
        return Err(format!(
            "no release keystore at {}; set BE3_ANDROID_RELEASE_KEYSTORE or run deploy at a terminal to make or import one",
            saved.display()
        ));
    }
    eprintln!("Android updates must use the same signing key. Keep a backup of this key.");
    eprintln!(
        "1) generate a new release key\n2) import an existing JKS/PKCS12 file or its base64 encoding (password android)"
    );
    eprint!("Keystore [1-2]: ");
    std::io::stderr()
        .flush()
        .map_err(|error| error.to_string())?;
    let mut choice = String::new();
    std::io::stdin()
        .read_line(&mut choice)
        .map_err(|error| error.to_string())?;
    match choice.trim() {
        "1" => generate_key(module, &saved, 4096)?,
        "2" => {
            eprint!("Keystore file: ");
            std::io::stderr()
                .flush()
                .map_err(|error| error.to_string())?;
            let mut path = String::new();
            std::io::stdin()
                .read_line(&mut path)
                .map_err(|error| error.to_string())?;
            let path = path.trim();
            let path = if let Some(rest) = path.strip_prefix("~/") {
                PathBuf::from(
                    std::env::var_os("HOME")
                        .or_else(|| std::env::var_os("USERPROFILE"))
                        .ok_or("HOME or USERPROFILE is required")?,
                )
                .join(rest)
            } else {
                path.into()
            };
            let bytes = read_key(&path)?;
            save_key(&saved, &bytes)?;
        }
        _ => return Err("pick 1 or 2".into()),
    }
    Ok(saved)
}

fn read_key(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if bytes.first() == Some(&0x30) || bytes.starts_with(&[0xfe, 0xed, 0xfe, 0xed]) {
        return Ok(bytes);
    }
    let encoded: Vec<u8> = bytes
        .into_iter()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| {
            format!(
                "{}: expected JKS, PKCS12 or base64 of one: {error}",
                path.display()
            )
        })
}

fn save_key(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::create_dir_all(parent(path)).map_err(|error| error.to_string())?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if path
        .extension()
        .is_some_and(|extension| extension == "base64")
    {
        file.write_all(
            base64::engine::general_purpose::STANDARD
                .encode(bytes)
                .as_bytes(),
        )
    } else {
        file.write_all(bytes)
    }
    .map_err(|error| error.to_string())
}

pub(crate) fn generate_key(module: &Path, path: &Path, bits: u32) -> Result<(), String> {
    let scratch = Scratch::new(parent(path))?;
    execute(
        module,
        &[
            "apksign".into(),
            "--generate-key".into(),
            "/work/key".into(),
            bits.to_string(),
        ],
        &scratch.0,
        None,
    )?;
    save_key(
        path,
        &fs::read(scratch.0.join("key")).map_err(|error| error.to_string())?,
    )?;
    println!("Made Android signing key at {}", path.display());
    Ok(())
}

fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

pub(crate) fn finalize(
    module: &Path,
    input: &Path,
    output: &Path,
    key: &Path,
    metadata: &Path,
) -> Result<(), String> {
    sign(module, input, output, key, Some(metadata), b"android")
}

fn sign(
    module: &Path,
    input: &Path,
    output: &Path,
    key: &Path,
    metadata: Option<&Path>,
    password: &[u8],
) -> Result<(), String> {
    let input = fs::canonicalize(input).map_err(|error| format!("{}: {error}", input.display()))?;
    let scratch = Scratch::new(parent(output))?;
    fs::write(scratch.0.join("key"), read_key(key)?).map_err(|error| error.to_string())?;
    fs::write(scratch.0.join("password"), password).map_err(|error| error.to_string())?;
    let input = if let Some(metadata) = metadata {
        let staged = scratch.0.join("input.apk");
        fs::copy(&input, &staged).map_err(|error| error.to_string())?;
        append_metadata(&staged, metadata)?;
        staged
    } else {
        input
    };
    let input_name = input
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("input filename is not UTF-8")?;
    let args = [
        "apksign".into(),
        "-in".into(),
        format!("/input/{input_name}"),
        "-out".into(),
        "/work/signed.apk".into(),
        "-keystore".into(),
        "/work/key".into(),
        "-storepass".into(),
        "file:/work/password".into(),
    ];
    execute(module, &args, &scratch.0, Some(&input))?;
    fs::rename(scratch.0.join("signed.apk"), output).map_err(|error| error.to_string())?;
    println!("Wrote {}", output.display());
    Ok(())
}

fn execute(
    module: &Path,
    args: &[String],
    directory: &Path,
    input: Option<&Path>,
) -> Result<(), String> {
    let mut config = Config::new();
    config.cache(Some(
        Cache::from_file(None).map_err(|error| error.to_string())?,
    ));
    let engine = Engine::new(&config).map_err(|error| error.to_string())?;
    let module = Module::from_file(&engine, module).map_err(|error| error.to_string())?;
    let mut wasi = WasiCtxBuilder::new();
    wasi.inherit_stdout().inherit_stderr().args(args);
    if let Some(input) = input {
        wasi.preopened_dir(parent(input), "/input", FsPerms::ReadOnly)
            .map_err(|error| error.to_string())?;
    }
    wasi.preopened_dir(directory, "/work", FsPerms::ReadWrite)
        .map_err(|error| error.to_string())?;
    let mut store = Store::new(&engine, wasi.build_p1());
    let mut linker = Linker::new(&engine);
    p1::add_to_linker_sync(&mut linker, |ctx| ctx).map_err(|error| error.to_string())?;
    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|error| error.to_string())?;
    let start = instance
        .get_typed_func::<(), ()>(&mut store, "_start")
        .map_err(|error| error.to_string())?;
    if let Err(error) = start.call(&mut store, ())
        && !error
            .downcast_ref::<wasmtime_wasi::I32Exit>()
            .is_some_and(|exit| exit.0 == 0)
    {
        return Err(error.to_string());
    }
    Ok(())
}
fn append_metadata(apk: &Path, metadata: &Path) -> Result<(), String> {
    use std::io::Write;
    use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

    let mut archive = ZipArchive::new(fs::File::open(apk).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    if archive.by_name("assets/build-info.json").is_ok() {
        return Err(
            "APK already contains assets/build-info.json; use a template without build metadata"
                .into(),
        );
    }
    drop(archive);
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(apk)
        .map_err(|error| error.to_string())?;
    let mut archive = ZipWriter::new_append(file).map_err(|error| error.to_string())?;
    archive
        .start_file(
            "assets/build-info.json",
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )
        .map_err(|error| error.to_string())?;
    archive
        .write_all(&fs::read(metadata).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    archive.finish().map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests;
