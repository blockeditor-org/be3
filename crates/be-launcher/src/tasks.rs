use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread;

use beui::Waker;

const PULL_REQUEST_LIMIT: &str = "100";
const READ_CHUNK: usize = 4096;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PullRequest {
    pub(crate) branch: String,
    pub(crate) label: String,
}

pub(crate) struct Listing {
    pub(crate) pull_requests: Vec<PullRequest>,
    pub(crate) source: &'static str,
}

pub(crate) enum Event {
    PullRequests(Result<Listing, String>),
    Head(String),
    Started(String),
    Output(Vec<u8>),
    Finished { summary: String, success: bool },
}

#[derive(Clone)]
pub(crate) struct Tasks {
    root: PathBuf,
    sender: Sender<Event>,
    waker: Arc<OnceLock<Waker>>,
    running: Arc<Mutex<Option<u32>>>,
}

impl Tasks {
    pub(crate) fn new(root: PathBuf, sender: Sender<Event>) -> Self {
        Self {
            root,
            sender,
            waker: Arc::new(OnceLock::new()),
            running: Arc::new(Mutex::new(None)),
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

    pub(crate) fn list_pull_requests(&self) {
        let tasks = self.clone();
        thread::spawn(move || {
            let listed = tasks
                .pull_requests_from_github()
                .map(|pull_requests| Listing {
                    pull_requests,
                    source: "open pull requests",
                })
                .or_else(|_| {
                    tasks.remote_branches().map(|pull_requests| Listing {
                        pull_requests,
                        source: "remote branches (gh is not available)",
                    })
                });
            tasks.send(Event::PullRequests(listed));
        });
    }

    fn pull_requests_from_github(&self) -> Result<Vec<PullRequest>, String> {
        let listed = capture(
            &self.root,
            "gh",
            &[
                "pr",
                "list",
                "--limit",
                PULL_REQUEST_LIMIT,
                "--json",
                "number,title,headRefName",
                "--template",
                "{{range .}}{{.headRefName}}\t#{{.number}} {{.title}}\n{{end}}",
            ],
        )?;
        Ok(parse_pull_requests(&listed))
    }

    fn remote_branches(&self) -> Result<Vec<PullRequest>, String> {
        capture(&self.root, "git", &["fetch", "--prune", "origin"])?;
        let listed = capture(
            &self.root,
            "git",
            &[
                "for-each-ref",
                "--sort=-committerdate",
                "--format=%(refname:lstrip=3)",
                "refs/remotes/origin",
            ],
        )?;
        Ok(parse_branches(&listed))
    }

    pub(crate) fn read_head(&self) {
        let tasks = self.clone();
        thread::spawn(move || {
            let head = capture(&tasks.root, "git", &["log", "-1", "--format=%h %s"])
                .map(|head| format!("At {}", head.trim()))
                .unwrap_or_else(|error| error);
            tasks.send(Event::Head(head));
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
        let mut command = command(bash());
        command
            .arg(format!("scripts/{script}"))
            .args(args)
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        let mut child = command
            .spawn()
            .map_err(|error| format!("could not start bash: {error}"))?;
        *self.lock_running() = Some(child.id());
        let readers = [
            child.stdout.take().map(|out| self.forward(out)),
            child.stderr.take().map(|err| self.forward(err)),
        ];
        for reader in readers.into_iter().flatten() {
            let _ = reader.join();
        }
        let status = child.wait();
        *self.lock_running() = None;
        let status = status.map_err(|error| error.to_string())?;
        Ok((status.to_string(), status.success()))
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
        let Some(process) = *self.lock_running() else {
            return;
        };
        let _ = kill_tree(process);
    }

    fn lock_running(&self) -> MutexGuard<'_, Option<u32>> {
        self.running
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
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

fn command(program: impl AsRef<OsStr>) -> Command {
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

fn capture(root: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    let output = command(program)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
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

pub(crate) fn repository_root() -> Result<PathBuf, String> {
    let root = capture(Path::new("."), "git", &["rev-parse", "--show-toplevel"])
        .map_err(|error| format!("run the launcher from inside the be3 checkout: {error}"))?;
    Ok(PathBuf::from(root.trim()))
}

pub(crate) fn parse_pull_requests(listed: &str) -> Vec<PullRequest> {
    listed
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(branch, label)| PullRequest {
            branch: branch.to_owned(),
            label: label.to_owned(),
        })
        .collect()
}

pub(crate) fn parse_branches(listed: &str) -> Vec<PullRequest> {
    listed
        .lines()
        .map(str::trim)
        .filter(|branch| !branch.is_empty() && *branch != "HEAD")
        .map(|branch| PullRequest {
            branch: branch.to_owned(),
            label: branch.to_owned(),
        })
        .collect()
}
