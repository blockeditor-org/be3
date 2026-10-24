use std::cell::RefCell;
use std::path::{Path, PathBuf};

use beui::reactive::*;

#[cfg(target_os = "android")]
use crate::android;
use crate::builds::{self, Installed, Slot};
use crate::helper::{self, Helper, Manifest, Product, State};
use crate::net::Network;
use crate::pane::Pane;
use crate::tasks::{self, Tasks};

const INSTALLED: &str = "installed.json";
const MANIFESTS: &str = "manifests";
const PROGRESS_STEP: u64 = 1 << 20;
const BOLD: &str = "\x1b[1m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

pub(crate) enum Event {
    Stage(Stage),
    Log(Vec<u8>),
    Ready(Result<Ready, String>),
    #[cfg(target_os = "android")]
    Committed(Result<(), String>),
    #[cfg(target_os = "android")]
    Finished(Result<(), String>),
}

pub(crate) struct Ready {
    installed: Installed,
    #[cfg(target_os = "android")]
    file: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Stage {
    Asking,
    Queued(u32),
    Building,
    Downloading {
        done: u64,
        total: u64,
    },
    #[cfg(target_os = "android")]
    Removing,
    #[cfg(target_os = "android")]
    Installing,
    #[cfg(not(target_os = "android"))]
    Starting,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Work {
    pub(crate) slot: Slot,
    pub(crate) product: Product,
    pub(crate) stage: Stage,
    pub(crate) fresh: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct Runs {
    tasks: Stored<Tasks>,
    pub(crate) work: Signal<Option<Work>>,
    pub(crate) problem: Signal<Option<(Slot, String)>>,
    pub(crate) installed: Signal<Option<Installed>>,
    pub(crate) logged: Signal<Option<Slot>>,
    pub(crate) needs_key: Signal<bool>,
    pub(crate) pane: Pane,
    pending: Stored<RefCell<Option<Ready>>>,
}

impl Runs {
    pub(crate) fn new(tasks: Tasks, pane: Pane) -> Self {
        #[cfg(target_os = "android")]
        android::remember(tasks.clone());
        let installed = builds::installed(&tasks.root().join(INSTALLED));
        #[cfg(target_os = "android")]
        let installed = installed.filter(|_| android::installed());
        let needs_key = helper::key(tasks.root()).is_none();
        Self {
            tasks: store(tasks),
            work: create_signal(None),
            problem: create_signal(None),
            installed: create_signal(installed),
            logged: create_signal(None),
            needs_key: create_signal(needs_key),
            pane,
            pending: store(RefCell::default()),
        }
    }

    pub(crate) fn holds(&self, slot: Slot) -> bool {
        self.installed.with(|installed| {
            installed
                .as_ref()
                .is_some_and(|installed| installed.slot == slot)
        })
    }

    pub(crate) fn busy(&self) -> bool {
        self.work.with(Option::is_some)
    }

    pub(crate) fn save_key(&self, key: &str) {
        match helper::save_key(self.tasks.value().root(), key) {
            Ok(()) => self.needs_key.set(false),
            Err(error) => self.pane.write_line(&format!("{RED}{error}{RESET}")),
        }
    }

    pub(crate) fn run(&self, slot: Slot, commit: Option<String>, product: Product, fresh: bool) {
        if self.busy() {
            return;
        }
        self.problem.set(None);
        self.pending.value().replace(None);
        self.logged.set(Some(slot));
        self.work.set(Some(Work {
            slot,
            product,
            stage: Stage::Asking,
            fresh,
        }));
        self.tasks.value().background(move |tasks| {
            tasks::Event::Run(Event::Ready(build(tasks, slot, commit, product)))
        });
    }

    fn stage(&self, stage: Stage) {
        self.work.update(|work| {
            if let Some(work) = work {
                work.stage = stage;
            }
        });
    }

    fn fail(&self, error: String) {
        if error == helper::UNAUTHORIZED || error == helper::NO_KEY {
            self.needs_key.set(true);
        }
        self.pending.value().replace(None);
        self.pane.write_line(&format!("{RED}{error}{RESET}\r\n"));
        let slot = self.work.get_untracked().map(|work| work.slot);
        self.work.set(None);
        if let Some(slot) = slot {
            self.problem.set(Some((slot, error)));
        }
    }

    fn done(&self, installed: Installed) {
        let record = self.tasks.value().root().join(INSTALLED);
        if let Err(error) = builds::remember(&record, Some(&installed)) {
            self.fail(error);
            return;
        }
        self.pane
            .write_line(&format!("{GREEN}Started {}{RESET}\r\n", installed.product));
        self.installed.set(Some(installed));
        self.work.set(None);
    }

    pub(crate) fn receive(&self, event: Event) {
        match event {
            Event::Stage(stage) => self.stage(stage),
            Event::Log(bytes) => self.pane.write(&bytes),
            Event::Ready(Err(error)) => self.fail(error),
            Event::Ready(Ok(ready)) => self.finish(ready),
            #[cfg(target_os = "android")]
            Event::Committed(Ok(())) => {}
            #[cfg(target_os = "android")]
            Event::Committed(Err(error)) | Event::Finished(Err(error)) => self.fail(error),
            #[cfg(target_os = "android")]
            Event::Finished(Ok(())) => self.installed_one(),
        }
    }

    #[cfg(not(target_os = "android"))]
    fn finish(&self, ready: Ready) {
        self.done(ready.installed);
    }

    #[cfg(target_os = "android")]
    fn finish(&self, ready: Ready) {
        let fresh = self
            .work
            .with_untracked(|work| work.as_ref().is_some_and(|work| work.fresh));
        let launcher = ready.installed.product == Product::Launcher.name();
        let file = ready.file.clone();
        self.pending.value().replace(Some(ready));
        if fresh && !launcher {
            self.stage(Stage::Removing);
            self.installed.set(None);
            let _ = builds::remember(&self.tasks.value().root().join(INSTALLED), None);
            self.tasks
                .value()
                .background(|_| tasks::Event::Run(Event::Committed(android::uninstall())));
        } else {
            self.install(file);
        }
    }

    #[cfg(target_os = "android")]
    fn install(&self, file: PathBuf) {
        self.stage(Stage::Installing);
        self.tasks
            .value()
            .background(move |_| tasks::Event::Run(Event::Committed(android::install(&file))));
    }

    #[cfg(target_os = "android")]
    fn installed_one(&self) {
        let Some(work) = self.work.get_untracked() else {
            return;
        };
        if work.stage == Stage::Removing {
            let file = self
                .pending
                .value()
                .borrow()
                .as_ref()
                .map(|ready| ready.file.clone());
            if let Some(file) = file {
                self.install(file);
            }
            return;
        }
        let Some(ready) = self.pending.value().borrow_mut().take() else {
            self.work.set(None);
            return;
        };
        if work.product == Product::Launcher {
            self.work.set(None);
            return;
        }
        self.done(ready.installed);
        if let Err(error) = android::open() {
            self.problem.set(Some((work.slot, error)));
        }
    }
}

fn build(
    tasks: &Tasks,
    slot: Slot,
    commit: Option<String>,
    product: Product,
) -> Result<Ready, String> {
    let commit = match commit {
        Some(commit) => commit,
        None => tasks.github()?.branch_head("main")?,
    };
    let say = |text: String| tasks.send(tasks::Event::Run(Event::Log(text.into_bytes())));
    say(format!(
        "{BOLD}Building {} at {commit}{RESET}\r\n",
        product.title()
    ));
    let cache = tasks.root().to_path_buf();
    let key = helper::key(&cache).ok_or(helper::NO_KEY)?;
    let mut http = Network;
    let mut helper = Helper {
        http: &mut http,
        key,
    };
    let mut status = helper.request(product, &commit)?;
    let mut from = 0;
    loop {
        let stage = match status.state {
            State::Queued => Stage::Queued(status.position.unwrap_or(0)),
            State::Running => Stage::Building,
            State::Succeeded | State::Failed => break,
            State::Refused | State::Unknown => {
                return Err(status
                    .error
                    .unwrap_or_else(|| "The build helper will not build it.".to_owned()));
            }
        };
        tasks.send(tasks::Event::Run(Event::Stage(stage)));
        let bytes = helper.log(product, &commit, from)?;
        from += bytes.len();
        if !bytes.is_empty() {
            tasks.send(tasks::Event::Run(Event::Log(bytes)));
        }
        status = helper.status(product, &commit)?;
    }
    loop {
        let bytes = helper.log(product, &commit, from).unwrap_or_default();
        if bytes.is_empty() {
            break;
        }
        from += bytes.len();
        tasks.send(tasks::Event::Run(Event::Log(bytes)));
    }
    if status.state == State::Failed {
        return Err(status
            .error
            .unwrap_or_else(|| "The build failed.".to_owned()));
    }
    let url = status
        .manifest
        .ok_or("The build helper named no manifest.")?;
    let manifest = helper::manifest(&mut http, &url)?;
    let mut reported = 0;
    let mut progress = |done: u64, total: u64| {
        if done == total || done >= reported + PROGRESS_STEP {
            reported = done;
            tasks.send(tasks::Event::Run(Event::Stage(Stage::Downloading {
                done,
                total,
            })));
        }
    };
    builds::fetch(&cache, &manifest, &mut http, &mut progress)?;
    keep(&cache, &manifest)?;
    let installed = Installed {
        slot,
        commit,
        product: manifest.product.clone(),
    };
    let executable = product.executable();
    let file = manifest
        .files
        .iter()
        .find(|file| file.path == executable)
        .ok_or_else(|| format!("The build has no {executable}."))?;
    #[cfg(target_os = "android")]
    return Ok(Ready {
        file: builds::cached(&cache, &file.sha256),
        installed,
    });
    #[cfg(not(target_os = "android"))]
    {
        let _ = file;
        tasks.send(tasks::Event::Run(Event::Stage(Stage::Starting)));
        let directory = cache.join("apps").join(&manifest.product);
        builds::assemble(&cache, &manifest, &directory)?;
        crate::tasks::start(&directory.join(&executable))?;
        Ok(Ready { installed })
    }
}

fn keep(cache: &Path, manifest: &Manifest) -> Result<(), String> {
    let manifests = cache.join(MANIFESTS);
    std::fs::create_dir_all(&manifests).map_err(|error| error.to_string())?;
    let document = serde_json::to_vec(manifest).map_err(|error| error.to_string())?;
    std::fs::write(
        manifests.join(format!("{}.json", manifest.product)),
        document,
    )
    .map_err(|error| error.to_string())?;
    let kept: Vec<Manifest> = std::fs::read_dir(&manifests)
        .map_err(|error| error.to_string())?
        .flatten()
        .filter_map(|entry| std::fs::read(entry.path()).ok())
        .filter_map(|document| Manifest::parse(&document).ok())
        .collect();
    builds::forget_unused(cache, &kept.iter().collect::<Vec<_>>());
    Ok(())
}

#[cfg(not(target_os = "android"))]
pub(crate) fn cache_directory() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .unwrap_or_else(std::env::temp_dir)
        .join("be3")
        .join("launcher")
}
