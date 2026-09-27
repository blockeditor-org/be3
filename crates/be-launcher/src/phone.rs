use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use beui::NodeId;
use beui::icons::ICON_PLAY_ARROW;
use beui::reactive::{
    Align, Direction, ItemSize, List, Memo, ReadSignal, Show, Text, WriteSignal, clone, component,
    create_memo, create_signal, view,
};
use beui::styled::theme::FONT_BODY;
use beui::styled::{Button, ButtonVariant, Caption, MenuButton, Spinner, use_theme};
use beui::unstyled::MenuItem;

use crate::android;
use crate::builds::{self, Build, Installed, Object, Slot};
use crate::github::{Entry, PullRequest};
use crate::model::{Cache, Loaded, Model};
use crate::tasks::{self, Tasks};

const APP: &str = "app.apk";
const LAUNCHER: &str = "launcher.apk";
const INSTALLED: &str = "installed.json";
const PROGRESS_STEP: u64 = 1 << 20;
const MEGABYTE: f64 = 1_000_000.0;
const SHORT_SHA: usize = 7;

pub(crate) enum Event {
    Found(Option<Installed>),
    Fetched(Slot, Result<Option<Build>, String>),
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

#[derive(Clone)]
pub(crate) struct Phone {
    tasks: Tasks,
    builds: Cache<Slot, Option<Build>>,
    installed: ReadSignal<Option<Installed>>,
    set_installed: WriteSignal<Option<Installed>>,
    work: ReadSignal<Option<Work>>,
    set_work: WriteSignal<Option<Work>>,
    problem: ReadSignal<Option<(Slot, String)>>,
    set_problem: WriteSignal<Option<(Slot, String)>>,
    pending: Rc<RefCell<Option<Installed>>>,
}

impl Phone {
    pub(crate) fn new(tasks: Tasks) -> Self {
        android::remember(tasks.clone());
        let (installed, set_installed) = create_signal(None);
        let (work, set_work) = create_signal(None);
        let (problem, set_problem) = create_signal(None);
        Self {
            tasks,
            builds: Rc::new(RefCell::new(HashMap::new())),
            installed,
            set_installed,
            work,
            set_work,
            problem,
            set_problem,
            pending: Rc::default(),
        }
    }

    pub(crate) fn start(&self) {
        self.tasks.background(|tasks| {
            let record = android::files(tasks).join(INSTALLED);
            let installed = builds::installed(&record).filter(|_| android::installed());
            tasks::Event::Phone(Event::Found(installed))
        });
    }

    pub(crate) fn holds(&self, slot: Slot) -> bool {
        self.installed.with(|installed| {
            installed
                .as_ref()
                .is_some_and(|installed| installed.slot == slot)
        })
    }

    pub(crate) fn build(&self, slot: Slot, commit: &str) -> ReadSignal<Loaded<Option<Build>>> {
        if let Some((read, _)) = self.builds.borrow().get(&slot) {
            return read.clone();
        }
        let (read, write) = create_signal(Loaded::Loading);
        self.builds.borrow_mut().insert(slot, (read.clone(), write));
        self.fetch(slot, commit.to_owned());
        read
    }

    pub(crate) fn refresh(&self, slot: Slot, commit: &str) {
        let write = self
            .builds
            .borrow()
            .get(&slot)
            .map(|(_, write)| write.clone());
        if let Some(write) = write {
            write.set(Loaded::Loading);
            self.fetch(slot, commit.to_owned());
        }
    }

    fn fetch(&self, slot: Slot, commit: String) {
        self.tasks.background(move |_| {
            tasks::Event::Phone(Event::Fetched(slot, fetch_build(slot, &commit)))
        });
    }

    pub(crate) fn run(&self, slot: Slot, commit: String, fresh: bool) {
        if !self.begin(slot, Package::App, fresh) {
            return;
        }
        self.tasks.background(move |tasks| {
            let files = android::files(tasks);
            let result = fetch_build(slot, &commit).and_then(|build| {
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

    pub(crate) fn install(&self, slot: Slot, commit: String) {
        if !self.begin(slot, Package::Launcher, false) {
            return;
        }
        self.tasks.background(move |tasks| {
            let apk = android::files(tasks).join(LAUNCHER);
            let result = fetch_build(slot, &commit).and_then(|build| {
                let build = build.ok_or_else(|| "be3-ci has no Android build of it".to_owned())?;
                let mut progress = progress(tasks, slot);
                builds::fetch_apk(
                    &apk,
                    slot,
                    &build.launcher,
                    &mut android::Http,
                    &mut progress,
                )?;
                Ok(Downloaded::Launcher)
            });
            tasks::Event::Phone(Event::Downloaded(slot, result))
        });
    }

    fn begin(&self, slot: Slot, package: Package, fresh: bool) -> bool {
        if self.work.with(Option::is_some) {
            return false;
        }
        self.pending.replace(None);
        self.set_problem.set(None);
        self.set_work.set(Some(Work {
            slot,
            package,
            stage: Stage::Downloading { done: 0, total: 0 },
            fresh,
        }));
        true
    }

    fn stage(&self, stage: Stage) {
        self.set_work.update(|work| {
            if let Some(work) = work {
                work.stage = stage;
            }
        });
    }

    fn fail(&self, error: String) {
        self.pending.replace(None);
        let slot = self.work.get_untracked().map(|work| work.slot);
        self.set_work.set(None);
        if let Some(slot) = slot {
            self.set_problem.set(Some((slot, error)));
        }
    }

    fn open(&self) {
        let slot = self.work.get_untracked().map(|work| work.slot);
        self.set_work.set(None);
        if let (Err(error), Some(slot)) = (android::open(), slot) {
            self.set_problem.set(Some((slot, error)));
        }
    }

    fn install_apk(&self, name: &'static str) {
        self.stage(Stage::Installing);
        self.tasks.background(move |tasks| {
            let apk = android::files(tasks).join(name);
            tasks::Event::Phone(Event::Committed(android::install(&apk)))
        });
    }

    fn remove_app(&self) {
        self.stage(Stage::Removing);
        self.tasks
            .background(|_| tasks::Event::Phone(Event::Committed(android::uninstall())));
    }

    fn forget(&self) -> Result<(), String> {
        self.set_installed.set(None);
        builds::remember(&android::files(&self.tasks).join(INSTALLED), None)
    }

    pub(crate) fn receive(&self, event: Event) {
        match event {
            Event::Found(installed) => self.set_installed.set(installed),
            Event::Fetched(slot, build) => {
                let write = self
                    .builds
                    .borrow()
                    .get(&slot)
                    .map(|(_, write)| write.clone());
                if let Some(write) = write {
                    write.set(match build {
                        Ok(build) => Loaded::Ready(build),
                        Err(error) => Loaded::Failed(error),
                    });
                }
            }
            Event::Progress(slot, done, total) => {
                self.set_work.update(|work| {
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
                    self.pending.replace(Some(Installed {
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
                        let installed = self.pending.borrow_mut().take();
                        if let Some(installed) = installed {
                            let record = android::files(&self.tasks).join(INSTALLED);
                            if let Err(error) = builds::remember(&record, Some(&installed)) {
                                self.fail(error);
                                return;
                            }
                            self.set_installed.set(Some(installed));
                        }
                        self.open();
                    }
                    _ => self.set_work.set(None),
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

fn fetch_build(slot: Slot, commit: &str) -> Result<Option<Build>, String> {
    match android::get(&slot.manifest_url(commit))? {
        Some(document) => Build::parse(&document).map(Some),
        None => Ok(None),
    }
}

fn now_key() -> String {
    let minutes = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() / 60);
    format!("t{minutes}")
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
    let phone = model.phone.clone();
    let initial = pull_request.get_untracked();
    let slot = Slot::PullRequest(initial.number);
    let build = phone.build(slot, &initial.head_sha);
    let busy = create_memo(clone!(phone -> move || phone.work.with(Option::is_some)));
    let disabled = create_memo(clone!(build busy -> move || {
        busy.get() || !matches!(build.get(), Loaded::Ready(Some(_)))
    }));
    let run = clone!(phone pull_request -> move || {
        phone.run(slot, pull_request.get_untracked().head_sha, false);
    });
    let pick = clone!(phone pull_request -> move |path: Vec<usize>| {
        let head = pull_request.get_untracked().head_sha;
        match path.as_slice() {
            [0] => phone.run(slot, head, true),
            [1] => phone.install(slot, head),
            _ => {}
        }
    });
    let menu_disabled = disabled.clone();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=1.0>
            <Button
                label="Run"
                glyph=ICON_PLAY_ARROW
                variant=ButtonVariant::Primary
                disabled
                on_click={run}
            />
            <MenuButton
                label="More ways to run it"
                variant=ButtonVariant::Primary
                icon_only=true
                disabled={menu_disabled}
                items={view! {
                    <MenuItem label="Run with fresh data" />
                    <MenuItem label="Install its launcher" />
                }}
                on_select={pick}
            />
        </List>
    }
}

#[component]
pub(crate) fn Notes(
    model: Model,
    pull_request: Memo<PullRequest>,
    timeline: ReadSignal<Loaded<Vec<Entry>>>,
) -> NodeId {
    let _ = timeline;
    let phone = model.phone.clone();
    let initial = pull_request.get_untracked();
    let slot = Slot::PullRequest(initial.number);
    let build = phone.build(slot, &initial.head_sha);
    let status = create_memo(clone!(build pull_request -> move || {
        let pull_request = pull_request.get();
        match build.get() {
            Loaded::Loading => "Looking for its Android build".to_owned(),
            Loaded::Failed(error) => error,
            Loaded::Ready(None) if !pull_request.same_repository => {
                "It comes from a fork, so CI does not publish an Android build of it.".to_owned()
            }
            Loaded::Ready(None) => "CI has not published an Android build of it yet.".to_owned(),
            Loaded::Ready(Some(build)) if build.commit == pull_request.head_sha => format!(
                "The Android build of {} is {}.",
                short(&build.commit),
                megabytes(build.app.size)
            ),
            Loaded::Ready(Some(build)) => format!(
                "CI has not published {} yet, so this runs {}.",
                short(&pull_request.head_sha),
                short(&build.commit)
            ),
        }
    }));
    view! {
        <List spacing=8.0>
            <Caption content={status} wrap=true />
            <SlotNotes model slot />
        </List>
    }
}

#[component]
pub(crate) fn MainActions(model: Model) -> NodeId {
    let phone = model.phone.clone();
    let busy = create_memo(clone!(phone -> move || phone.work.with(Option::is_some)));
    let menu_busy = busy.clone();
    let run = clone!(phone -> move || phone.run(Slot::Main, now_key(), false));
    let pick = clone!(phone -> move |path: Vec<usize>| match path.as_slice() {
        [0] => phone.run(Slot::Main, now_key(), true),
        [1] => phone.install(Slot::Main, now_key()),
        _ => {}
    });
    let current = create_memo(clone!(phone -> move || phone.holds(Slot::Main)));
    let summary = create_memo(clone!(current -> move || if current.get() {
        "main is installed"
    } else {
        "The latest build of main"
    }.to_owned()));
    view! {
        <List spacing=8.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Caption @sizing=ItemSize::Percent(100.0) content={summary} />
                <List direction=Direction::Horizontal align=Align::Center spacing=1.0>
                    <Button
                        label="Run main"
                        glyph=ICON_PLAY_ARROW
                        variant=ButtonVariant::Secondary
                        disabled={busy}
                        on_click={run}
                    />
                    <MenuButton
                        label="More ways to run main"
                        variant=ButtonVariant::Secondary
                        icon_only=true
                        disabled={menu_busy}
                        items={view! {
                            <MenuItem label="Run main with fresh data" />
                            <MenuItem label="Install main's launcher" />
                        }}
                        on_select={pick}
                    />
                </List>
            </List>
            <SlotNotes model slot={Slot::Main} />
        </List>
    }
}

#[component]
fn SlotNotes(model: Model, slot: Slot) -> NodeId {
    let theme = use_theme();
    let phone = model.phone.clone();
    let working = create_memo(clone!(phone -> move || phone.work.with(|work| {
        work.as_ref().is_some_and(|work| work.slot == slot)
    })));
    let progress = create_memo(clone!(phone -> move || match phone.work.get() {
        Some(Work { stage: Stage::Downloading { total: 0, .. }, .. }) => "Looking for the build".to_owned(),
        Some(Work { stage: Stage::Downloading { done, total }, .. }) => {
            format!("Downloading {} of {}", megabytes(done), megabytes(total))
        }
        Some(Work { stage: Stage::Removing, .. }) => "Removing the app and its data".to_owned(),
        Some(Work { stage: Stage::Installing, package: Package::App, .. }) => "Installing the app".to_owned(),
        Some(Work { stage: Stage::Installing, package: Package::Launcher, .. }) => {
            "Installing the launcher".to_owned()
        }
        None => String::new(),
    }));
    let problem = create_memo(clone!(phone -> move || match phone.problem.get() {
        Some((problem_slot, problem)) if problem_slot == slot => problem,
        _ => String::new(),
    }));
    let failed = create_memo(clone!(problem -> move || !problem.get().is_empty()));
    view! {
        <List spacing=8.0>
            <Show condition={working}>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Spinner width=18.0 label="Working on the build" />
                    <Caption content={progress} />
                </List>
            </Show>
            <Show condition={failed}>
                <Text string={problem} font_size=FONT_BODY color={theme.danger.clone()} wrap=true />
            </Show>
        </List>
    }
}
