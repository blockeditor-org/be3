use std::fs::{self, File};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::{Unanswered, request};

const USAGE: &str = "usage: be-drive launch --program=PATH [--name=NAME] [--data] [--libraries=DIR]
       [--size=WIDTHxHEIGHT[@SCALE]] [--screen=WIDTHxHEIGHT] [--fresh] [--stop] [PROGRAM ARGUMENTS...]
Starts a beui program headless with an automation socket, waits until it settles, and writes
an env file that puts `drive` on PATH pointed at it. Arguments it does not know go to the program.";
const STARTUP: Duration = Duration::from_secs(120);
const SIZE: &str = "1100x720";
const SOCKET_PATH: usize = 107;
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

const RUN_USAGE: &str =
    "usage: be-drive run --program=PATH [--name=NAME] [--data] [--libraries=DIR]
       [--size=WIDTHxHEIGHT[@SCALE]] [--screen=WIDTHxHEIGHT] [PROGRAM ARGUMENTS...] < COMMANDS
Starts a beui program headless as launch does, with fresh data and its output on this one's,
runs COMMANDS against it one a line as `drive -` does, then waits for it to exit. It fails
unless every command succeeded and the program exited cleanly, so COMMANDS end with close.";
const EXIT: Duration = Duration::from_secs(60);

fn parse(arguments: &[String], usage: &str) -> Result<Launch, String> {
    let mut launch = Launch::default();
    for argument in arguments {
        let value = |prefix: &str| argument.strip_prefix(prefix).map(str::to_owned);
        match argument.as_str() {
            "--data" => launch.data = true,
            "--fresh" => launch.fresh = true,
            "--stop" => launch.stop = true,
            "--help" => return Err(usage.to_owned()),
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
    if launch.program.is_none() {
        return Err(usage.to_owned());
    }
    Ok(launch)
}

impl Launch {
    fn program(&self) -> &str {
        self.program.as_deref().unwrap_or_default()
    }

    fn name(&self) -> String {
        self.name.clone().unwrap_or_else(|| {
            Path::new(self.program()).file_name().map_or_else(
                || "app".to_owned(),
                |name| name.to_string_lossy().into_owned(),
            )
        })
    }

    fn start(&self, dir: &Path, log: Option<&Path>) -> Result<Child, String> {
        let name = self.name();
        let program = self.program();
        let data = dir.join("data");
        if self.fresh && data.exists() {
            fs::remove_dir_all(&data)
                .map_err(|error| format!("could not delete {}: {error}", data.display()))?;
        }
        let socket = dir.join("automation.sock");
        let length = socket.as_os_str().len();
        if length > SOCKET_PATH {
            return Err(format!(
                "{} is {length} bytes, longer than the {SOCKET_PATH} a Unix socket's path may be; \
                 set BE_DRIVE_DIR to a shorter directory",
                socket.display()
            ));
        }
        let _ = fs::remove_file(&socket);
        let listener = UnixListener::bind(&socket)
            .map_err(|error| format!("could not listen on {}: {error}", socket.display()))?;
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
        let listening = listener.as_raw_fd();
        let mut command = Command::new(program);
        command
            .args(&self.arguments)
            .env("BEUI_HEADLESS", self.size.as_deref().unwrap_or(SIZE))
            .env(
                "BEUI_HEADLESS_SCREEN",
                self.screen.as_deref().unwrap_or(SCREEN),
            )
            .env("BEUI_AUTOMATION", &socket)
            .env("BEUI_AUTOMATION_LISTENER", listening.to_string())
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .stdin(Stdio::null())
            .process_group(0);
        // unsafe(ffi)
        unsafe {
            command.pre_exec(move || match libc::fcntl(listening, libc::F_SETFD, 0) {
                -1 => Err(std::io::Error::last_os_error()),
                _ => Ok(()),
            });
        }
        if let Some(log) = log {
            let file = File::create(log)
                .map_err(|error| format!("could not create {}: {error}", log.display()))?;
            command
                .stdout(file.try_clone().map_err(|error| error.to_string())?)
                .stderr(file);
        }
        if let Some(libraries) = &self.libraries {
            command.env("LD_LIBRARY_PATH", libraries);
        }
        if self.data {
            command.env("XDG_DATA_HOME", &data);
        }
        let see = log.map_or_else(String::new, |log| format!("; see {}", log.display()));
        let spawned = command.spawn();
        drop(listener);
        let mut child = spawned.map_err(|error| format!("could not start {program}: {error}"))?;
        if let Err(error) = request(&socket, &["settle".to_owned()], STARTUP) {
            kill_group(child.id());
            let status = child.wait().map_err(|error| error.to_string())?;
            return Err(match error {
                Unanswered::Closed => {
                    format!("{name} exited ({status}) before it answered; is it a beui app?{see}")
                }
                Unanswered::Failed(error) => format!("{error}{see}"),
            });
        }
        Ok(child)
    }
}

pub fn run(arguments: &[String]) -> Result<(), String> {
    let launch = parse(arguments, USAGE)?;
    let name = launch.name();
    let dir = directory(&name)?;
    fs::create_dir_all(&dir)
        .map_err(|error| format!("could not create {}: {error}", dir.display()))?;
    stop(&dir.join("app.pid"), launch.program());
    if launch.stop {
        println!("Stopped {name}.");
        return Ok(());
    }
    let drive = install_drive(&dir)?;
    let log_path = dir.join("app.log");
    let child = launch.start(&dir, Some(&log_path))?;
    fs::write(dir.join("app.pid"), child.id().to_string()).map_err(|error| error.to_string())?;
    let env = dir.join("env");
    let bin = drive.parent().map_or_else(PathBuf::new, Path::to_path_buf);
    fs::write(
        &env,
        format!(
            "export BE_DRIVE_SOCKET={}\nexport PATH={}:\"$PATH\"\n",
            quoted(&dir.join("automation.sock")),
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

pub(crate) fn start(
    program: &str,
    arguments: Vec<String>,
    size: &str,
    libraries: Option<String>,
    dir: &Path,
    log: &Path,
) -> Result<Child, String> {
    Launch {
        program: Some(program.to_owned()),
        data: true,
        fresh: true,
        libraries,
        size: Some(size.to_owned()),
        arguments,
        ..Launch::default()
    }
    .start(dir, Some(log))
}

pub(crate) fn wait_for_exit(mut child: Child, timeout: Duration) -> Result<(), String> {
    let pid = child.id();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(child.wait());
    });
    match receiver.recv_timeout(timeout) {
        Ok(Ok(status)) if status.success() => Ok(()),
        Ok(Ok(status)) => Err(format!("the app exited with {status}")),
        Ok(Err(error)) => Err(format!("could not wait for the app: {error}")),
        Err(_) => {
            kill_group(pid);
            Err(format!(
                "the app did not exit within {} seconds of closing",
                timeout.as_secs()
            ))
        }
    }
}

pub(crate) fn kill(pid: u32) {
    kill_group(pid);
}

pub fn run_script(arguments: &[String]) -> Result<(), String> {
    let mut launch = parse(arguments, RUN_USAGE)?;
    launch.fresh = true;
    let name = launch.name();
    let dir = directory(&name)?;
    fs::create_dir_all(&dir)
        .map_err(|error| format!("could not create {}: {error}", dir.display()))?;
    let mut child = launch.start(&dir, None)?;
    if let Err(error) = crate::script(&dir.join("automation.sock"), &[], crate::TIMEOUT) {
        kill_group(child.id());
        let _ = child.wait();
        return Err(error);
    }
    let pid = child.id();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(child.wait());
    });
    match receiver.recv_timeout(EXIT) {
        Ok(Ok(status)) if status.success() => {
            println!("{name} exited cleanly.");
            Ok(())
        }
        Ok(Ok(status)) => Err(format!("{name} exited with {status}")),
        Ok(Err(error)) => Err(format!("could not wait for {name}: {error}")),
        Err(_) => {
            kill_group(pid);
            Err(format!(
                "{name} did not exit within {} seconds of the last command; end the commands with close",
                EXIT.as_secs()
            ))
        }
    }
}

fn kill_group(pid: u32) {
    let _ = Command::new("kill")
        .args(["-9", "--", &format!("-{pid}")])
        .status();
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
