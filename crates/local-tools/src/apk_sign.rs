use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::atomic::{AtomicU64, Ordering},
};

use wasmtime::{Cache, Config, Engine, Linker, Module, Store};
use wasmtime_wasi::{FsPerms, WasiCtxBuilder, p1};

const USAGE: &str = "usage: buck run //buck/android:sign-wasm -- --in APK --out APK --keystore FILE [--password-file FILE] [--build-info JSON]";

struct Scratch(PathBuf);

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
    let module = arguments.next().ok_or(USAGE)?;
    let (mut input, mut output, mut key, mut password, mut metadata) =
        (None, None, None, None, None);
    while let Some(argument) = arguments.next() {
        let value = arguments.next().ok_or(USAGE)?;
        match argument.as_str() {
            "--in" => input = Some(PathBuf::from(value)),
            "--out" => output = Some(PathBuf::from(value)),
            "--keystore" => key = Some(PathBuf::from(value)),
            "--password-file" => password = Some(PathBuf::from(value)),
            "--build-info" => metadata = Some(PathBuf::from(value)),
            _ => return Err(USAGE.into()),
        }
    }
    let input = fs::canonicalize(input.ok_or(USAGE)?).map_err(|error| error.to_string())?;
    let output = output.ok_or(USAGE)?;
    let key = key.ok_or(USAGE)?;
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
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
    let scratch = Scratch(directory);
    fs::copy(&key, scratch.0.join("key")).map_err(|error| error.to_string())?;
    let password = match password {
        Some(path) => fs::read(path).map_err(|error| error.to_string())?,
        None => b"android".to_vec(),
    };
    fs::write(scratch.0.join("password"), password).map_err(|error| error.to_string())?;
    let input = if let Some(metadata) = metadata {
        let staged = scratch.0.join("input.apk");
        fs::copy(&input, &staged).map_err(|error| error.to_string())?;
        append_metadata(&staged, &metadata)?;
        staged
    } else {
        input
    };
    let mut config = Config::new();
    config.cache(Some(
        Cache::from_file(None).map_err(|error| error.to_string())?,
    ));
    let engine = Engine::new(&config).map_err(|error| error.to_string())?;
    let module = Module::from_file(&engine, module).map_err(|error| error.to_string())?;
    let input_name = input
        .file_name()
        .ok_or("input has no filename")?
        .to_str()
        .ok_or("input filename is not UTF-8")?;
    let args = [
        "apksign".to_owned(),
        "-v1".into(),
        "-in".into(),
        format!("/input/{input_name}"),
        "-out".into(),
        "/work/signed.apk".into(),
        "-keystore".into(),
        "/work/key".into(),
        "-storepass".into(),
        "file:/work/password".into(),
    ];
    let mut wasi = WasiCtxBuilder::new();
    wasi.inherit_stdout().inherit_stderr().args(&args);
    wasi.preopened_dir(
        input.parent().ok_or("input has no parent")?,
        "/input",
        FsPerms::ReadOnly,
    )
    .map_err(|error| error.to_string())?;
    wasi.preopened_dir(&scratch.0, "/work", FsPerms::ReadWrite)
        .map_err(|error| error.to_string())?;
    let mut store = Store::new(&engine, wasi.build_p1());
    let input_size = usize::try_from(
        fs::metadata(&input)
            .map_err(|error| error.to_string())?
            .len(),
    )
    .map_err(|error| error.to_string())?;
    store.set_hostcall_fuel(input_size.saturating_add(16 * 1024 * 1024));
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
    fs::rename(scratch.0.join("signed.apk"), &output).map_err(|error| error.to_string())?;
    println!("Wrote {}", output.display());
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
