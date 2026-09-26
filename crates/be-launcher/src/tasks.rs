use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread;

use beui::Waker;
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::github::{Entry, Filter, GitHub, PullRequest, download};
use crate::targets::{QUERY, Target, parse_targets};

const READ_CHUNK: usize = 4096;
const DEFAULT_COLS: u16 = 100;
const DEFAULT_ROWS: u16 = 30;
const MAX_IMAGE_WIDTH: u32 = 1200;

pub(crate) struct Pixels {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

pub(crate) enum Event {
    Connected(Result<String, String>),
    PullRequests(Filter, Result<Vec<PullRequest>, String>),
    Timeline(u64, Result<Vec<Entry>, String>),
    Image(String, Result<Pixels, String>),
    Head { sha: String, summary: String },
    Targets(Result<Vec<Target>, String>),
    Started(String),
    Output(Vec<u8>),
    Finished { summary: String, success: bool },
}

struct Running {
    process: Option<u32>,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
}

#[derive(Clone)]
pub(crate) struct Tasks {
    root: PathBuf,
    sender: Sender<Event>,
    waker: Arc<OnceLock<Waker>>,
    github: Arc<OnceLock<Result<GitHub, String>>>,
    running: Arc<Mutex<Option<Running>>>,
    size: Arc<Mutex<(u16, u16)>>,
}

impl Tasks {
    pub(crate) fn new(root: PathBuf, sender: Sender<Event>) -> Self {
        Self {
            root,
            sender,
            waker: Arc::new(OnceLock::new()),
            github: Arc::new(OnceLock::new()),
            running: Arc::new(Mutex::new(None)),
            size: Arc::new(Mutex::new((DEFAULT_COLS, DEFAULT_ROWS))),
        }
    }

    pub(crate) fn set_waker(&self, waker: Waker) {
        let _ = self.waker.set(waker);
    }

    fn send(&self, event: Event) {
        let _ = self.sender.send(event);
        if let Some(waker) = self.waker.get() {
            waker.wake();
        }
    }

    fn background(&self, work: impl FnOnce(&Tasks) -> Event + Send + 'static) {
        let tasks = self.clone();
        thread::spawn(move || {
            let event = work(&tasks);
            tasks.send(event);
        });
    }

    fn github(&self) -> Result<&GitHub, String> {
        self.github
            .get_or_init(|| GitHub::connect(&self.root))
            .as_ref()
            .map_err(Clone::clone)
    }

    pub(crate) fn connect(&self) {
        self.background(|tasks| {
            Event::Connected(tasks.github().map(|github| github.repository().full_name()))
        });
    }

    pub(crate) fn list(&self, filter: Filter) {
        self.background(move |tasks| {
            Event::PullRequests(
                filter,
                tasks
                    .github()
                    .and_then(|github| github.pull_requests(filter)),
            )
        });
    }

    pub(crate) fn timeline(&self, pull_request: PullRequest) {
        self.background(move |tasks| {
            Event::Timeline(
                pull_request.number,
                tasks
                    .github()
                    .and_then(|github| github.timeline(&pull_request)),
            )
        });
    }

    pub(crate) fn image(&self, url: String) {
        self.background(move |_| {
            let pixels = download(&url, None).and_then(|bytes| decode(&bytes));
            Event::Image(url, pixels)
        });
    }

    pub(crate) fn targets(&self) {
        self.background(|tasks| {
            let listed = capture(
                &tasks.root,
                bash(),
                &[
                    "scripts/buck",
                    "uquery",
                    QUERY,
                    "--output-attribute",
                    "^buck.type$",
                    "--output-attribute",
                    "^binary$",
                ],
            )
            .and_then(|listed| {
                serde_json::from_str(&listed)
                    .map_err(|error| format!("buck2 listed the targets as {error}"))
            });
            Event::Targets(listed.map(|listed| parse_targets(&listed)))
        });
    }

    pub(crate) fn read_head(&self) {
        self.background(|tasks| {
            let sha = capture(&tasks.root, "git", &["rev-parse", "HEAD"])
                .map(|sha| sha.trim().to_owned())
                .unwrap_or_default();
            let summary = capture(&tasks.root, "git", &["log", "-1", "--format=%h %s"])
                .map(|summary| summary.trim().to_owned())
                .unwrap_or_else(|error| error);
            Event::Head { sha, summary }
        });
    }

    pub(crate) fn run(&self, script: &'static str, args: Vec<String>) {
        let tasks = self.clone();
        thread::spawn(move || {
            let description = format!("./scripts/{script} {}", args.join(" "));
            tasks.send(Event::Started(description.clone()));
            let (summary, success) = match tasks.run_to_end(script, &args) {
                Ok((status, success)) => (format!("{description}: {status}"), success),
                Err(error) => (format!("{description}: {error}"), false),
            };
            tasks.send(Event::Finished { summary, success });
            tasks.read_head();
        });
    }

    fn run_to_end(&self, script: &str, args: &[String]) -> Result<(String, bool), String> {
        let (cols, rows) = *self.lock_size();
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("could not open a terminal: {error}"))?;
        let mut command = CommandBuilder::new(bash());
        command.arg(format!("scripts/{script}"));
        command.args(args);
        command.cwd(&self.root);
        command.env_clear();
        for (key, value) in std::env::vars_os() {
            command.env(key, value);
        }
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("could not start bash: {error}"))?;
        drop(pair.slave);
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| error.to_string())?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| error.to_string())?;
        *self.lock_running() = Some(Running {
            process: child.process_id(),
            master: pair.master,
            writer,
        });
        let forwarding = self.forward(reader);
        let status = child.wait();
        #[cfg(unix)]
        let _ = forwarding.join();
        let finished = self.lock_running().take();
        drop(finished);
        #[cfg(windows)]
        let _ = forwarding.join();
        let status = status.map_err(|error| error.to_string())?;
        let described = match status.exit_code() {
            0 => "finished".to_owned(),
            code => format!("exited with status {code}"),
        };
        Ok((described, status.success()))
    }

    fn forward(&self, mut stream: impl Read + Send + 'static) -> thread::JoinHandle<()> {
        let tasks = self.clone();
        thread::spawn(move || {
            let mut buffer = [0u8; READ_CHUNK];
            while let Ok(read) = stream.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                tasks.send(Event::Output(buffer[..read].to_vec()));
            }
        })
    }

    pub(crate) fn stop(&self) {
        let Some(process) = self
            .lock_running()
            .as_ref()
            .and_then(|running| running.process)
        else {
            return;
        };
        let _ = kill_tree(process);
    }

    pub(crate) fn input(&self, bytes: &[u8]) {
        if let Some(running) = self.lock_running().as_mut() {
            let _ = running.writer.write_all(bytes);
            let _ = running.writer.flush();
        }
    }

    pub(crate) fn resize(&self, cols: u16, rows: u16) {
        *self.lock_size() = (cols, rows);
        if let Some(running) = self.lock_running().as_ref() {
            let _ = running.master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
    }

    fn lock_running(&self) -> MutexGuard<'_, Option<Running>> {
        self.running
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_size(&self) -> MutexGuard<'_, (u16, u16)> {
        self.size
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn decode(bytes: &[u8]) -> Result<Pixels, String> {
    let decoded = image::load_from_memory(bytes).map_err(|error| error.to_string())?;
    let decoded = if decoded.width() > MAX_IMAGE_WIDTH {
        let height = (u64::from(decoded.height()) * u64::from(MAX_IMAGE_WIDTH)
            / u64::from(decoded.width()))
        .max(1) as u32;
        decoded.resize_exact(
            MAX_IMAGE_WIDTH,
            height,
            image::imageops::FilterType::Triangle,
        )
    } else {
        decoded
    };
    let rgba = decoded.to_rgba8();
    if rgba.width() == 0 || rgba.height() == 0 {
        return Err("the image is empty".to_owned());
    }
    Ok(Pixels {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

#[cfg(unix)]
fn kill_tree(group: u32) -> std::io::Result<std::process::ExitStatus> {
    command("kill")
        .args(["-TERM", "--", &format!("-{group}")])
        .status()
}

#[cfg(windows)]
fn kill_tree(process: u32) -> std::io::Result<std::process::ExitStatus> {
    command("taskkill")
        .args(["/T", "/F", "/PID", &process.to_string()])
        .status()
}

pub(crate) fn command(program: impl AsRef<OsStr>) -> Command {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::os::windows::process::CommandExt::creation_flags(&mut command, CREATE_NO_WINDOW);
    }
    command
}

#[cfg(not(windows))]
fn bash() -> PathBuf {
    PathBuf::from("bash")
}

#[cfg(windows)]
fn bash() -> PathBuf {
    std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .map(|directory| directory.join("bash.exe"))
        .find(|candidate| candidate.is_file() && !is_windows_launcher(candidate))
        .unwrap_or_else(|| PathBuf::from("bash"))
}

#[cfg(windows)]
fn is_windows_launcher(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy().to_ascii_lowercase();
        name == "system32" || name == "windowsapps"
    })
}

pub(crate) fn capture(
    root: &Path,
    program: impl AsRef<OsStr>,
    args: &[&str],
) -> Result<String, String> {
    let program = program.as_ref();
    let output = command(program)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("{}: {error}", program.to_string_lossy()))?;
    if !output.status.success() {
        return Err(format!(
            "{} failed: {}",
            program.to_string_lossy(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub(crate) fn repository_root() -> Result<PathBuf, String> {
    let root = capture(Path::new("."), "git", &["rev-parse", "--show-toplevel"])
        .map_err(|error| format!("run the launcher from inside the be3 checkout: {error}"))?;
    Ok(PathBuf::from(root.trim()))
}

pub(crate) fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let spawned = command("open").arg(url).spawn();
    #[cfg(windows)]
    let spawned = command("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(not(any(target_os = "macos", windows)))]
    let spawned = command("xdg-open").arg(url).spawn();
    if let Err(error) = spawned {
        eprintln!("could not open {url}: {error}");
    }
}
