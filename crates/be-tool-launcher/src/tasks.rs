use std::path::{Path, PathBuf};
#[cfg(not(target_os = "android"))]
use std::process::{Child, Command};
#[cfg(not(target_os = "android"))]
use std::sync::Mutex;
use std::sync::mpsc::Sender;
use std::sync::{Arc, OnceLock};
use std::thread;

use beui::native::Waker;

use crate::github::{Entry, Filter, GitHub, PullRequest, download};
use crate::runs;

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
    Run(runs::Event),
}

#[derive(Clone)]
pub(crate) struct Tasks {
    root: PathBuf,
    sender: Sender<Event>,
    waker: Arc<OnceLock<Waker>>,
    github: Arc<OnceLock<Result<GitHub, String>>>,
}

impl Tasks {
    pub(crate) fn new(root: PathBuf, sender: Sender<Event>) -> Self {
        Self {
            root,
            sender,
            waker: Arc::new(OnceLock::new()),
            github: Arc::new(OnceLock::new()),
        }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn set_waker(&self, waker: Waker) {
        let _ = self.waker.set(waker);
    }

    pub(crate) fn send(&self, event: Event) {
        let _ = self.sender.send(event);
        if let Some(waker) = self.waker.get() {
            waker.wake();
        }
    }

    pub(crate) fn background(&self, work: impl FnOnce(&Tasks) -> Event + Send + 'static) {
        let tasks = self.clone();
        thread::spawn(move || {
            let event = work(&tasks);
            tasks.send(event);
        });
    }

    pub(crate) fn github(&self) -> Result<&GitHub, String> {
        self.github
            .get_or_init(GitHub::connect)
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

#[cfg(not(target_os = "android"))]
static STARTED: Mutex<Option<Child>> = Mutex::new(None);

#[cfg(not(target_os = "android"))]
pub(crate) fn start(program: &Path) -> Result<(), String> {
    let mut started = STARTED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(mut previous) = started.take() {
        let _ = previous.kill();
        let _ = previous.wait();
    }
    let child = command(program)
        .current_dir(program.parent().unwrap_or(Path::new(".")))
        .spawn()
        .map_err(|error| format!("could not start {}: {error}", program.display()))?;
    *started = Some(child);
    Ok(())
}

#[cfg(not(target_os = "android"))]
pub(crate) fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    for variable in beui_host::native::environment::VARIABLES {
        command.env_remove(variable);
    }
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::os::windows::process::CommandExt::creation_flags(&mut command, CREATE_NO_WINDOW);
    }
    command
}

#[cfg(not(target_os = "android"))]
pub(crate) fn capture(program: &str, args: &[&str]) -> Result<String, String> {
    let output = command(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|error| format!("{program}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(target_os = "android")]
pub(crate) fn open_url(url: &str) {
    crate::android::open_url(url);
}

#[cfg(not(target_os = "android"))]
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
