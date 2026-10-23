use std::cell::RefCell;
use std::collections::HashMap;

use beui::NodeId;
use beui::icons::ICON_PLAY;
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::MenuItem;

use crate::android;
use crate::builds::{self, Build, Installed, Object, Slot};
use crate::github::{Entry, PullRequest};
use crate::model::{Cache, Loaded, Model};
use crate::tasks::{self, Tasks};

const APP: &str = "app.apk";
const LAUNCHER: &str = "launcher.apk";
const INSTALLED: &str = "installed.json";
const LAUNCHER_RECORD: &str = "launcher.json";
const PROGRESS_STEP: u64 = 1 << 20;
const MEGABYTE: f64 = 1_000_000.0;
const SHORT_SHA: usize = 7;

pub(crate) enum Event {
    Found(Option<Installed>, Own),
    Fetched(Slot, Result<Found, String>),
    Progress(Slot, u64, u64),
    Downloaded(Slot, Result<Downloaded, String>),
    Committed(Result<(), String>),
    Finished(Result<(), String>),
}

pub(crate) enum Downloaded {
    Current,
    App { commit: String, app: Object },
    Launcher,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Found {
    pub(crate) wanted: String,
    pub(crate) build: Option<Build>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Own {
    pub(crate) hash: String,
    pub(crate) record: Option<Installed>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Package {
    App,
    Launcher,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Stage {
    Downloading { done: u64, total: u64 },
    Removing,
    Installing,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Work {
    pub(crate) slot: Slot,
    pub(crate) package: Package,
    pub(crate) stage: Stage,
    pub(crate) fresh: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct Phone {
    tasks: Stored<Tasks>,
    builds: Cache<Slot, Found>,
    own: Signal<Option<Own>>,
    installed: Signal<Option<Installed>>,
    work: Signal<Option<Work>>,
    problem: Signal<Option<(Slot, String)>>,
    pending: Stored<RefCell<Option<Installed>>>,
}

impl Phone {
    pub(crate) fn new(tasks: Tasks) -> Self {
        android::remember(tasks.clone());
        let installed = create_signal(None);
        let own = create_signal(None);
        let work = create_signal(None);
        let problem = create_signal(None);
        Self {
            tasks: store(tasks),
            builds: store(RefCell::new(HashMap::new())),
            own,
            installed,
            work,
            problem,
            pending: store(RefCell::default()),
        }
    }

    pub(crate) fn start(&self) {
        self.tasks.value().background(|tasks| {
            let files = android::files(tasks);
            let installed =
                builds::installed(&files.join(INSTALLED)).filter(|_| android::installed());
            let hash = android::apk()
                .and_then(|apk| builds::hash_file(&apk))
                .unwrap_or_default();
            let record = builds::installed(&files.join(LAUNCHER_RECORD))
                .filter(|record| !hash.is_empty() && record.hash == hash);
            tasks::Event::Phone(Event::Found(installed, Own { hash, record }))
        });
    }

    pub(crate) fn holds(&self, slot: Slot) -> bool {
        self.installed.with(|installed| {
            installed
                .as_ref()
                .is_some_and(|installed| installed.slot == slot)
        })
    }

    pub(crate) fn build(&self, slot: Slot, commit: &str) -> ReadSignal<Loaded<Found>> {
        if let Some(loaded) = self.builds.value().borrow().get(&slot) {
            return loaded.read_only();
        }
        let loaded = create_signal(Loaded::Loading);
        self.builds.value().borrow_mut().insert(slot, loaded);
        self.fetch(slot, Some(commit.to_owned()));
        loaded.read_only()
    }

    pub(crate) fn main_build(&self) -> ReadSignal<Loaded<Found>> {
        if let Some(loaded) = self.builds.value().borrow().get(&Slot::Main) {
            return loaded.read_only();
        }
        let loaded = create_signal(Loaded::Loading);
        self.builds.value().borrow_mut().insert(Slot::Main, loaded);
        self.fetch(Slot::Main, None);
        loaded.read_only()
    }

    pub(crate) fn refresh(&self, slot: Slot, commit: &str) {
        self.refetch(slot, Some(commit.to_owned()));
    }

    pub(crate) fn refresh_main(&self) {
        self.refetch(Slot::Main, None);
    }

    fn refetch(&self, slot: Slot, commit: Option<String>) {
        let loaded = self.builds.value().borrow().get(&slot).copied();
        if let Some(loaded) = loaded {
            loaded.set(Loaded::Loading);
            self.fetch(slot, commit);
        }
    }

    fn fetch(&self, slot: Slot, commit: Option<String>) {
        self.tasks.value().background(move |tasks| {
            let found = commit
                .map_or_else(
                    || tasks.github().and_then(|github| github.branch_head("main")),
                    Ok,
                )
                .and_then(|wanted| {
                    let build = fetch_build(slot)?;
                    Ok(Found { wanted, build })
                });
            tasks::Event::Phone(Event::Fetched(slot, found))
        });
    }

    pub(crate) fn run(&self, slot: Slot, fresh: bool) {
        if !self.begin(slot, Package::App, fresh) {
            return;
        }
        self.tasks.value().background(move |tasks| {
            let files = android::files(tasks);
            let result = fetch_build(slot).and_then(|build| {
                let build = build.ok_or_else(|| "be3-ci has no Android build of it".to_owned())?;
                let current = builds::installed(&files.join(INSTALLED))
                    .is_some_and(|installed| installed.hash == build.app.hash);
                if current && !fresh && android::installed() {
                    return Ok(Downloaded::Current);
                }
                let mut progress = progress(tasks, slot);
                builds::fetch_apk(
                    &files.join(APP),
                    slot,
                    &build.app,
                    &mut android::Http,
                    &mut progress,
                )?;
                Ok(Downloaded::App {
                    commit: build.commit,
                    app: build.app,
                })
            });
            tasks::Event::Phone(Event::Downloaded(slot, result))
        });
    }

    pub(crate) fn install(&self, slot: Slot) {
        if !self.begin(slot, Package::Launcher, false) {
            return;
        }
        self.tasks.value().background(move |tasks| {
            let files = android::files(tasks);
            let result = fetch_build(slot).and_then(|build| {
                let build = build.ok_or_else(|| "be3-ci has no Android build of it".to_owned())?;
                let mut progress = progress(tasks, slot);
                builds::fetch_apk(
                    &files.join(LAUNCHER),
                    slot,
                    &build.launcher,
                    &mut android::Http,
                    &mut progress,
                )?;
                let record = Installed {
                    slot,
                    commit: build.commit,
                    hash: build.launcher.hash,
                };
                builds::remember(&files.join(LAUNCHER_RECORD), Some(&record))?;
                Ok(Downloaded::Launcher)
            });
            tasks::Event::Phone(Event::Downloaded(slot, result))
        });
    }

    fn begin(&self, slot: Slot, package: Package, fresh: bool) -> bool {
        if self.work.with(Option::is_some) {
            return false;
        }
        self.pending.value().replace(None);
        self.problem.set(None);
        self.work.set(Some(Work {
            slot,
            package,
            stage: Stage::Downloading { done: 0, total: 0 },
            fresh,
        }));
        true
    }

    fn stage(&self, stage: Stage) {
        self.work.update(|work| {
            if let Some(work) = work {
                work.stage = stage;
            }
        });
    }

    fn fail(&self, error: String) {
        self.pending.value().replace(None);
        let slot = self.work.get_untracked().map(|work| work.slot);
        self.work.set(None);
        if let Some(slot) = slot {
            self.problem.set(Some((slot, error)));
        }
    }

    fn open(&self) {
        let slot = self.work.get_untracked().map(|work| work.slot);
        self.work.set(None);
        if let (Err(error), Some(slot)) = (android::open(), slot) {
            self.problem.set(Some((slot, error)));
        }
    }

    fn install_apk(&self, name: &'static str) {
        self.stage(Stage::Installing);
        self.tasks.value().background(move |tasks| {
            let apk = android::files(tasks).join(name);
            tasks::Event::Phone(Event::Committed(android::install(&apk)))
        });
    }

    fn remove_app(&self) {
        self.stage(Stage::Removing);
        self.tasks
            .value()
            .background(|_| tasks::Event::Phone(Event::Committed(android::uninstall())));
    }

    fn forget(&self) -> Result<(), String> {
        self.installed.set(None);
        builds::remember(&android::files(&self.tasks.value()).join(INSTALLED), None)
    }

    pub(crate) fn receive(&self, event: Event) {
        match event {
            Event::Found(installed, own) => {
                self.installed.set(installed);
                self.own.set(Some(own));
            }
            Event::Fetched(slot, build) => {
                let loaded = self.builds.value().borrow().get(&slot).copied();
                if let Some(loaded) = loaded {
                    loaded.set(match build {
                        Ok(build) => Loaded::Ready(build),
                        Err(error) => Loaded::Failed(error),
                    });
                }
            }
            Event::Progress(slot, done, total) => {
                self.work.update(|work| {
                    if let Some(work) = work
                        && work.slot == slot
                        && matches!(work.stage, Stage::Downloading { .. })
                    {
                        work.stage = Stage::Downloading { done, total };
                    }
                });
            }
            Event::Downloaded(slot, result) => match result {
                Err(error) => self.fail(error),
                Ok(Downloaded::Current) => self.open(),
                Ok(Downloaded::App { commit, app }) => {
                    self.pending.value().replace(Some(Installed {
                        slot,
                        commit,
                        hash: app.hash,
                    }));
                    let fresh = self
                        .work
                        .with(|work| work.as_ref().is_some_and(|work| work.fresh));
                    if let Err(error) = self.forget() {
                        self.fail(error);
                    } else if fresh {
                        self.remove_app();
                    } else {
                        self.install_apk(APP);
                    }
                }
                Ok(Downloaded::Launcher) => self.install_apk(LAUNCHER),
            },
            Event::Committed(Ok(())) => {}
            Event::Committed(Err(error)) | Event::Finished(Err(error)) => self.fail(error),
            Event::Finished(Ok(())) => {
                let Some(work) = self.work.get_untracked() else {
                    return;
                };
                match (work.package, work.stage) {
                    (Package::App, Stage::Removing) => self.install_apk(APP),
                    (Package::App, Stage::Installing) => {
                        let installed = self.pending.value().borrow_mut().take();
                        if let Some(installed) = installed {
                            let record = android::files(&self.tasks.value()).join(INSTALLED);
                            if let Err(error) = builds::remember(&record, Some(&installed)) {
                                self.fail(error);
                                return;
                            }
                            self.installed.set(Some(installed));
                        }
                        self.open();
                    }
                    _ => self.work.set(None),
                }
            }
        }
    }
}

fn progress(tasks: &Tasks, slot: Slot) -> impl FnMut(u64, u64) + '_ {
    let mut reported = 0;
    move |done: u64, total: u64| {
        if done == total || done >= reported + PROGRESS_STEP {
            reported = done;
            tasks.send(tasks::Event::Phone(Event::Progress(slot, done, total)));
        }
    }
}

fn fetch_build(slot: Slot) -> Result<Option<Build>, String> {
    android::get(&slot.manifest_url())?
        .map(|document| Build::parse(&document))
        .transpose()
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(SHORT_SHA)]
}

fn megabytes(bytes: u64) -> String {
    format!("{:.0} MB", bytes as f64 / MEGABYTE)
}

#[component]
pub(crate) fn Actions(
    model: Model,
    pull_request: Memo<PullRequest>,
    timeline: ReadSignal<Loaded<Vec<Entry>>>,
) -> NodeId {
    let _ = timeline;
    let phone = model.phone;
    let initial = pull_request.get_untracked();
    let slot = Slot::PullRequest(initial.number);
    let build = phone.build(slot, &initial.head_sha);
    let busy = create_memo(move || phone.work.with(Option::is_some));
    let disabled = create_memo(move || busy.get() || !published(build));
    let run = move || phone.run(slot, false);
    let pick = move |path: Vec<usize>| match path.as_slice() {
        [0] => phone.run(slot, true),
        [1] => phone.install(slot),
        _ => {}
    };
    view! {
        <SplitButton
            label="Run"
            glyph=ICON_PLAY
            variant=ButtonVariant::Primary
            menu_label="More ways to run it"
            disabled
            items={view! {
                <MenuItem label="Run with fresh data" />
                <MenuItem label="Install its launcher" />
            }}
            on_click={run}
            on_select={pick}
        />
    }
}

#[component]
pub(crate) fn Notes(
    model: Model,
    pull_request: Memo<PullRequest>,
    timeline: ReadSignal<Loaded<Vec<Entry>>>,
) -> NodeId {
    let _ = timeline;
    let phone = model.phone;
    let initial = pull_request.get_untracked();
    let slot = Slot::PullRequest(initial.number);
    let build = phone.build(slot, &initial.head_sha);
    let status = create_memo(move || match build.get() {
        Loaded::Ready(Found { build: None, .. }) if !pull_request.get().same_repository => {
            "It comes from a fork, so CI does not publish an Android build of it.".to_owned()
        }
        found => describe(found, "Looking for its Android build", "it"),
    });
    view! {
        <List spacing=8.0>
            <Text style=TextStyle::CAPTION wrap=true>{status}</Text>
            <SlotNotes model slot />
        </List>
    }
}

#[component]
pub(crate) fn MainActions(model: Model) -> NodeId {
    let phone = model.phone;
    let build = phone.main_build();
    let busy = create_memo(move || phone.work.with(Option::is_some));
    let disabled = create_memo(move || busy.get() || !published(build));
    let run = move || phone.run(Slot::Main, false);
    let pick = move |path: Vec<usize>| match path.as_slice() {
        [0] => phone.run(Slot::Main, true),
        [1] => phone.install(Slot::Main),
        _ => {}
    };
    let current = create_memo(move || phone.holds(Slot::Main));
    let summary = create_memo(move || {
        let described = describe(build.get(), "Looking for main's Android build", "main");
        if current.get() {
            format!("main is installed. {described}")
        } else {
            described
        }
    });
    let launcher = create_memo(move || {
        let main = match build.get() {
            Loaded::Ready(Found {
                build: Some(build), ..
            }) => Some(build),
            _ => None,
        };
        match phone.own.get() {
            None => "Looking at which launcher this is".to_owned(),
            Some(Own {
                record: Some(record),
                ..
            }) => {
                format!(
                    "This launcher is {} at {}.",
                    slot_name(record.slot),
                    short(&record.commit)
                )
            }
            Some(Own { hash, .. }) => match main {
                Some(main) if !hash.is_empty() && main.launcher.hash == hash => {
                    format!("This launcher is main at {}.", short(&main.commit))
                }
                _ if hash.is_empty() => {
                    "Android did not say where this launcher is installed.".to_owned()
                }
                _ => format!(
                    "This launcher, {}, is not a build be3-ci has published.",
                    short(&hash)
                ),
            },
        }
    });
    view! {
        <List spacing=8.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Text style=TextStyle::CAPTION @sizing=ItemSize::Percent(100.0) wrap=true>
                    {summary}
                </Text>
                <SplitButton
                    label="Run main"
                    glyph=ICON_PLAY
                    variant=ButtonVariant::Secondary
                    menu_label="More ways to run main"
                    disabled
                    items={view! {
                        <MenuItem label="Run main with fresh data" />
                        <MenuItem label="Install main's launcher" />
                    }}
                    on_click={run}
                    on_select={pick}
                />
            </List>
            <Text style=TextStyle::CAPTION wrap=true>{launcher}</Text>
            <SlotNotes model slot={Slot::Main} />
        </List>
    }
}

fn published(build: ReadSignal<Loaded<Found>>) -> bool {
    matches!(build.get(), Loaded::Ready(Found { build: Some(_), .. }))
}

fn describe(found: Loaded<Found>, loading: &str, subject: &str) -> String {
    match found {
        Loaded::Loading => loading.to_owned(),
        Loaded::Failed(error) => error,
        Loaded::Ready(Found { build: None, .. }) => {
            format!("CI has not published an Android build of {subject} yet.")
        }
        Loaded::Ready(Found {
            wanted,
            build: Some(build),
        }) if build.commit == wanted => format!(
            "The Android build of {} is {}.",
            short(&build.commit),
            megabytes(build.app.size)
        ),
        Loaded::Ready(Found {
            wanted,
            build: Some(build),
        }) => format!(
            "CI has not published {} yet, so this runs {}.",
            short(&wanted),
            short(&build.commit)
        ),
    }
}

fn slot_name(slot: Slot) -> String {
    match slot {
        Slot::PullRequest(number) => format!("#{number}"),
        Slot::Main => "main".to_owned(),
    }
}

#[component]
fn SlotNotes(model: Model, slot: Slot) -> NodeId {
    let phone = model.phone;
    let working = create_memo(move || {
        phone
            .work
            .with(|work| work.as_ref().is_some_and(|work| work.slot == slot))
    });
    let progress = create_memo(move || match phone.work.get() {
        Some(Work {
            stage: Stage::Downloading { total: 0, .. },
            ..
        }) => "Looking for the build".to_owned(),
        Some(Work {
            stage: Stage::Downloading { done, total },
            ..
        }) => {
            format!("Downloading {} of {}", megabytes(done), megabytes(total))
        }
        Some(Work {
            stage: Stage::Removing,
            ..
        }) => "Removing the app and its data".to_owned(),
        Some(Work {
            stage: Stage::Installing,
            package: Package::App,
            ..
        }) => "Installing the app".to_owned(),
        Some(Work {
            stage: Stage::Installing,
            package: Package::Launcher,
            ..
        }) => "Installing the launcher".to_owned(),
        None => String::new(),
    });
    let problem = create_memo(move || match phone.problem.get() {
        Some((problem_slot, problem)) if problem_slot == slot => problem,
        _ => String::new(),
    });
    let failed = create_memo(move || !problem.get().is_empty());
    view! {
        <List spacing=8.0>
            <Show condition={working}>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Spinner width=18.0 label="Working on the build" />
                    <Text style=TextStyle::CAPTION>{progress}</Text>
                </List>
            </Show>
            <Show condition={failed}>
                <Text color=TextColor::Danger wrap=true>{problem}</Text>
            </Show>
        </List>
    }
}
