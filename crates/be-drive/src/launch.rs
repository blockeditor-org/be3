use std::fs::{self, File};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::request;

const USAGE: &str = "usage: be-drive launch APP LIBRARIES [--fresh] [--stop] [--desktop] [--size=WIDTHxHEIGHT[@SCALE]]";
const STARTUP: Duration = Duration::from_secs(120);
const SIZE: &str = "1100x720";

pub fn run(arguments: &[String]) -> Result<(), String> {
    let [app, libraries, flags @ ..] = arguments else {
        return Err(USAGE.to_owned());
    };
    let mut fresh = false;
    let mut stop = false;
    let mut desktop = false;
    let mut size = SIZE.to_owned();
    for flag in flags {
        match flag.as_str() {
            "--fresh" => fresh = true,
            "--stop" => stop = true,
            "--desktop" => desktop = true,
            _ => match flag.strip_prefix("--size=") {
                Some(value) => size = value.to_owned(),
                None => return Err(format!("unknown argument {flag}\n{USAGE}")),
            },
        }
    }
    let dir = directory()?;
    fs::create_dir_all(&dir).map_err(|error| format!("could not create {}: {error}", dir.display()))?;
    stop_app(&dir.join("app.pid"));
    if stop {
        println!("Stopped the app.");
        return Ok(());
    }
    if fresh {
        let data = dir.join("data");
        if data.exists() {
            fs::remove_dir_all(&data)
                .map_err(|error| format!("could not delete {}: {error}", data.display()))?;
        }
    }
    let drive = install_drive(&dir)?;
    let socket = dir.join("automation.sock");
    let _ = fs::remove_file(&socket);
    let log_path = dir.join("app.log");
    let log = File::create(&log_path)
        .map_err(|error| format!("could not create {}: {error}", log_path.display()))?;
    let mut command = Command::new(app);
    command
        .arg("--dev-workspace")
        .arg(format!("--headless={size}"))
        .arg(format!("--automation={}", socket.display()));
    if desktop {
        command.arg("--desktop");
    }
    let mut child = command
        .env("LD_LIBRARY_PATH", libraries)
        .env("XDG_DATA_HOME", dir.join("data"))
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|error| error.to_string())?)
        .stderr(log)
        .process_group(0)
        .spawn()
        .map_err(|error| format!("could not start {app}: {error}"))?;
    fs::write(dir.join("app.pid"), child.id().to_string()).map_err(|error| error.to_string())?;
    let started = Instant::now();
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!(
                "the app exited ({status}) before it answered; see {}",
                log_path.display()
            ));
        }
        if UnixStream::connect(&socket).is_ok() {
            break;
        }
        if started.elapsed() > STARTUP {
            return Err(format!(
                "the app did not open {} within {} seconds; see {}",
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
    fs::write(
        &env,
        format!(
            "export BE_DRIVE_SOCKET={}\nexport PATH={}:\"$PATH\"\n",
            socket.display(),
            drive.parent().map_or_else(PathBuf::new, Path::to_path_buf).display(),
        ),
    )
    .map_err(|error| format!("could not write {}: {error}", env.display()))?;
    println!(
        "The app is running headless (pid {}); guides/running_the_app.md says how to drive it.\n  source {}    puts `drive` on PATH, pointed at the app\n  log:  {}",
        child.id(),
        env.display(),
        log_path.display()
    );
    Ok(())
}

fn directory() -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os("BLOCK_DEV_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let cache = match std::env::var_os("XDG_CACHE_HOME") {
        Some(cache) => PathBuf::from(cache),
        None => PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set")?).join(".cache"),
    };
    Ok(cache.join("be3").join("dev"))
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

fn stop_app(pid_file: &Path) {
    let Ok(pid) = fs::read_to_string(pid_file) else {
        return;
    };
    let _ = fs::remove_file(pid_file);
    let Ok(pid) = pid.trim().parse::<i32>() else {
        return;
    };
    let comm = format!("/proc/{pid}/comm");
    let running = || fs::read_to_string(&comm).is_ok_and(|name| name.trim() == "block-app");
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
