use std::path::{Path, PathBuf};

use block_wasm_host::{Host, precompile, precompile_to};

fn main() {
    let mut arguments = std::env::args();
    let program = arguments
        .next()
        .unwrap_or_else(|| "plugin-test-runner".into());
    let Some(first) = arguments.next() else {
        usage(&program);
    };
    if first == "--precompile-to" {
        let artifact = arguments.next().map(PathBuf::from);
        let wasm = arguments.next().map(PathBuf::from);
        let (Some(artifact), Some(wasm)) = (artifact, wasm) else {
            usage(&program);
        };
        if let Err(message) = precompile_to(&wasm, &artifact, None) {
            eprintln!("{message}");
            std::process::exit(1);
        }
        return;
    }
    if first == "--precompile" {
        let modules: Vec<PathBuf> = arguments.map(PathBuf::from).collect();
        if modules.is_empty() {
            usage(&program);
        }
        if let Err(message) = precompile(&modules, None) {
            eprintln!("{message}");
            std::process::exit(1);
        }
        return;
    }
    let (first, precompiled) = if first == "--precompiled" {
        let artifact = arguments.next().map(PathBuf::from);
        let Some(next) = arguments.next() else {
            usage(&program);
        };
        (next, artifact)
    } else {
        (first, None)
    };
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

    match run(&wasm, &arguments, precompiled.as_deref()) {
        Ok(code) => std::process::exit(code),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}

fn usage(program: &str) -> ! {
    eprintln!("Usage: {program} [--precompiled TESTS.cwasm] TESTS.wasm [test arguments...]");
    eprintln!("       {program} --precompile TESTS.wasm...");
    eprintln!("       {program} --precompile-to TESTS.cwasm TESTS.wasm");
    std::process::exit(2);
}

fn run(wasm: &Path, arguments: &[String], precompiled: Option<&Path>) -> Result<i32, String> {
    let root = workspace()?;
    let host = Host::on_demand(gpu, None)?;
    host.run_tests(wasm, arguments, &root, precompiled)
}

fn workspace() -> Result<PathBuf, String> {
    let start = match std::env::var_os("CARGO_MANIFEST_DIR") {
        Some(directory) => PathBuf::from(directory),
        None => std::env::current_dir()
            .map_err(|error| format!("the working directory is unreadable: {error}"))?,
    };
    start
        .ancestors()
        .find(|directory| holds_the_workspace(directory))
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            format!(
                "no workspace holds {}, run these tests through cargo",
                start.display()
            )
        })
}

fn holds_the_workspace(directory: &Path) -> bool {
    std::fs::read_to_string(directory.join("Cargo.toml"))
        .is_ok_and(|manifest| manifest.contains("[workspace]"))
}

fn gpu() -> Result<(wgpu::Device, wgpu::Queue), String> {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .map_err(|error| format!("no graphics adapter is available: {error}"))?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("plugin test device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .map_err(|error| format!("the adapter did not provide a device: {error}"))
}
