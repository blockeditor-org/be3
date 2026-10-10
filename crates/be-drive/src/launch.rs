use std::fs::{self, File};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::request;

const USAGE: &str = "usage: be-drive launch --program=PATH [--name=NAME] [--data] [--libraries=DIR]
       [--size=WIDTHxHEIGHT[@SCALE]] [--screen=WIDTHxHEIGHT] [--fresh] [--stop] [PROGRAM ARGUMENTS...]
Starts a beui program headless with an automation socket, waits until it settles, and writes
an env file that puts `drive` on PATH pointed at it. Arguments it does not know go to the program.";
const STARTUP: Duration = Duration::from_secs(120);
const SIZE: &str = "1100x720";
const SCREEN: &str = "1440x900";

#[derive(Default)]
struct Launch {
    program: Option<String>,
    name: Option<String>,
    data: bool,
    libraries: Option<String>,
    size: Option<String>,
    screen: Option<String>,
    fresh: bool,
    stop: bool,
    arguments: Vec<String>,
}

pub fn run(arguments: &[String]) -> Result<(), String> {
    let mut launch = Launch::default();
    for argument in arguments {
        let value = |prefix: &str| argument.strip_prefix(prefix).map(str::to_owned);
        match argument.as_str() {
            "--data" => launch.data = true,
            "--fresh" => launch.fresh = true,
            "--stop" => launch.stop = true,
            "--help" => return Err(USAGE.to_owned()),
            _ => {
                if let Some(program) = value("--program=") {
                    launch.program = Some(program);
                } else if let Some(name) = value("--name=") {
                    launch.name = Some(name);
                } else if let Some(libraries) = value("--libraries=") {
                    launch.libraries = Some(libraries);
                } else if let Some(size) = value("--size=") {
                    launch.size = Some(size);
                } else if let Some(screen) = value("--screen=") {
                    launch.screen = Some(screen);
                } else {
                    launch.arguments.push(argument.clone());
                }
            }
        }
    }
    let program = launch.program.clone().ok_or(USAGE)?;
    let name = launch.name.clone().unwrap_or_else(|| {
        Path::new(&program).file_name().map_or_else(
            || "app".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        )
    });
    let dir = directory(&name)?;
    fs::create_dir_all(&dir)
        .map_err(|error| format!("could not create {}: {error}", dir.display()))?;
    stop(&dir.join("app.pid"), &program);
    if launch.stop {
        println!("Stopped {name}.");
        return Ok(());
    }
    let data = dir.join("data");
    if launch.fresh && data.exists() {
        fs::remove_dir_all(&data)
            .map_err(|error| format!("could not delete {}: {error}", data.display()))?;
    }
    let drive = install_drive(&dir)?;
    let socket = dir.join("automation.sock");
    let _ = fs::remove_file(&socket);
    let log_path = dir.join("app.log");
    let log = File::create(&log_path)
        .map_err(|error| format!("could not create {}: {error}", log_path.display()))?;
    let mut command = Command::new(&program);
    command
        .args(&launch.arguments)
        .env("BEUI_HEADLESS", launch.size.as_deref().unwrap_or(SIZE))
        .env(
            "BEUI_HEADLESS_SCREEN",
            launch.screen.as_deref().unwrap_or(SCREEN),
        )
        .env("BEUI_AUTOMATION", &socket)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|error| error.to_string())?)
        .stderr(log)
        .process_group(0);
    if let Some(libraries) = &launch.libraries {
        command.env("LD_LIBRARY_PATH", libraries);
    }
    if launch.data {
        command.env("XDG_DATA_HOME", &data);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("could not start {program}: {error}"))?;
    fs::write(dir.join("app.pid"), child.id().to_string()).map_err(|error| error.to_string())?;
    let started = Instant::now();
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!(
                "{name} exited ({status}) before it answered; see {}",
                log_path.display()
            ));
        }
        if UnixStream::connect(&socket).is_ok() {
            break;
        }
        if started.elapsed() > STARTUP {
            return Err(format!(
                "{name} did not open {} within {} seconds; is it a beui app? See {}",
                socket.display(),
                STARTUP.as_secs(),
                log_path.display()
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    request(&socket, &["settle".to_owned()], STARTUP)
        .map_err(|error| format!("{error}; see {}", log_path.display()))?;
    let env = dir.join("env");
    let bin = drive.parent().map_or_else(PathBuf::new, Path::to_path_buf);
    fs::write(
        &env,
        format!(
            "export BE_DRIVE_SOCKET={}\nexport PATH={}:\"$PATH\"\n",
            quoted(&socket),
            quoted(&bin),
        ),
    )
    .map_err(|error| format!("could not write {}: {error}", env.display()))?;
    println!(
        "{name} is running headless (pid {}); guides/running/drive.md says how to drive it.\n  source {}    puts `drive` on PATH, pointed at it\n  log:  {}",
        child.id(),
        quoted(&env),
        log_path.display()
    );
    Ok(())
}

fn quoted(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

fn directory(name: &str) -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os("BE_DRIVE_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let cache = match std::env::var_os("XDG_CACHE_HOME") {
        Some(cache) => PathBuf::from(cache),
        None => PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set")?).join(".cache"),
    };
    Ok(cache.join("be3").join(name))
}

fn install_drive(dir: &Path) -> Result<PathBuf, String> {
    let bin = dir.join("bin");
    fs::create_dir_all(&bin).map_err(|error| error.to_string())?;
    let drive = bin.join("drive");
    let me = std::env::current_exe().map_err(|error| error.to_string())?;
    let staged = bin.join("drive.new");
    fs::copy(&me, &staged)
        .and_then(|_| fs::rename(&staged, &drive))
        .map_err(|error| format!("could not install {}: {error}", drive.display()))?;
    Ok(drive)
}

fn stop(pid_file: &Path, program: &str) {
    let Ok(pid) = fs::read_to_string(pid_file) else {
        return;
    };
    let _ = fs::remove_file(pid_file);
    let Ok(pid) = pid.trim().parse::<i32>() else {
        return;
    };
    let name: String = Path::new(program)
        .file_name()
        .map(|name| name.to_string_lossy().chars().take(15).collect())
        .unwrap_or_default();
    let comm = format!("/proc/{pid}/comm");
    let running = || fs::read_to_string(&comm).is_ok_and(|comm| comm.trim() == name);
    if !running() {
        return;
    }
    let _ = Command::new("kill").arg(pid.to_string()).status();
    let started = Instant::now();
    while running() && started.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(50));
    }
    if running() {
        let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
    }
}
