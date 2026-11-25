use std::path::{Path, PathBuf};

use be_wasm_host::Host;

const GUEST_PATHS: [&str; 2] = ["CARGO_MANIFEST_DIR", "MADE_PAINTINGS"];

fn main() {
    absolute_guest_paths();
    let mut arguments = std::env::args();
    let program = arguments.next().unwrap_or_else(|| "app-test-runner".into());
    let Some(first) = arguments.next() else {
        usage(&program);
    };
    let mut libraries = Vec::new();
    let mut first = first;
    while first == "--library" {
        let (Some(path), Some(next)) = (arguments.next(), arguments.next()) else {
            usage(&program);
        };
        libraries.push(PathBuf::from(path));
        first = next;
    }
    let wasm = PathBuf::from(first);
    let mut arguments: Vec<String> = std::iter::once(wasm.to_string_lossy().into_owned())
        .chain(arguments)
        .collect();
    if !arguments
        .iter()
        .any(|argument| argument.starts_with("--test-threads"))
    {
        arguments.push("--test-threads=1".to_owned());
    }
    if !arguments.iter().any(|argument| argument == "--nocapture") {
        arguments.push("--nocapture".to_owned());
    }

    match run(&wasm, &arguments, &libraries) {
        Ok(code) => std::process::exit(code),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}

fn absolute_guest_paths() {
    let Ok(current) = std::env::current_dir() else {
        return;
    };
    for name in GUEST_PATHS {
        let Some(path) = std::env::var_os(name).map(PathBuf::from) else {
            continue;
        };
        if path.is_relative() {
            // unsafe(env)
            unsafe { std::env::set_var(name, current.join(path)) };
        }
    }
}

fn usage(program: &str) -> ! {
    eprintln!("Usage: {program} [--library LIBRARY.wasm]... TESTS.wasm [test arguments...]");
    std::process::exit(2);
}

fn run(wasm: &Path, arguments: &[String], libraries: &[PathBuf]) -> Result<i32, String> {
    let directories = guest_directories()?;
    let host = Host::on_demand(gpu)?;
    host.run_tests(wasm, arguments, &directories, libraries)
}

fn guest_directories() -> Result<Vec<PathBuf>, String> {
    let mut directories: Vec<PathBuf> = GUEST_PATHS
        .iter()
        .filter_map(|name| std::env::var_os(name).map(PathBuf::from))
        .collect();
    if std::env::var_os("CARGO_MANIFEST_DIR").is_none() {
        directories.push(
            std::env::current_dir()
                .map_err(|error| format!("the working directory is unreadable: {error}"))?,
        );
    }
    for directory in &directories {
        std::fs::create_dir_all(directory)
            .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
    }
    Ok(directories)
}

fn gpu() -> Result<(wgpu::Device, wgpu::Queue), String> {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
        compatible_surface: None,
    }))
    .map_err(|error| format!("no graphics adapter is available: {error}"))?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("app test device"),
        required_features: adapter.features() & wgpu::Features::all_webgpu_mask(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .map_err(|error| format!("the adapter did not provide a device: {error}"))
}
