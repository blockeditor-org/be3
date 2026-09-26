use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use beui::NodeId;
use beui::icons::{ICON_MORE_VERT, ICON_PLAY_ARROW, ICON_SYSTEM_UPDATE};
use beui::reactive::{
    Align, Direction, List, Memo, ReadSignal, Show, Text, WriteSignal, clone, component,
    create_memo, create_signal, view,
};
use beui::styled::theme::FONT_BODY;
use beui::styled::{Button, ButtonVariant, Caption, MenuButton, Spinner, use_theme};
use beui::unstyled::MenuItem;

use crate::android;
use crate::builds::{self, Build, Downloaded, Slot};
use crate::github::{Entry, PullRequest};
use crate::model::{Cache, Loaded, Model};
use crate::tasks::{self, Tasks};

const BUILD: &str = "build";
const DATA: &str = "data";
const LAUNCHER: &str = "launcher.apk";
const PROGRESS_STEP: u64 = 1 << 20;
const MEGABYTE: f64 = 1_000_000.0;
const SHORT_SHA: usize = 7;

pub(crate) enum Event {
    Found(Option<Downloaded>),
    Fetched(Slot, Result<Option<Build>, String>),
    Progress(Slot, u64, u64),
    Synced(Slot, Result<(), String>, Option<Downloaded>),
    Confirming,
    Installed(Result<(), String>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Stage {
    Downloading { done: u64, total: u64 },
    Installing,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Work {
    pub(crate) slot: Slot,
    pub(crate) stage: Stage,
}

#[derive(Clone)]
pub(crate) struct Phone {
    tasks: Tasks,
    shell: Rc<String>,
    builds: Cache<Slot, Option<Build>>,
    downloaded: ReadSignal<Option<Downloaded>>,
    set_downloaded: WriteSignal<Option<Downloaded>>,
    work: ReadSignal<Option<Work>>,
    set_work: WriteSignal<Option<Work>>,
    problem: ReadSignal<Option<(Slot, String)>>,
    set_problem: WriteSignal<Option<(Slot, String)>>,
}

impl Phone {
    pub(crate) fn new(tasks: Tasks, shell: String) -> Self {
        android::remember(tasks.clone());
        let (downloaded, set_downloaded) = create_signal(None);
        let (work, set_work) = create_signal(None);
        let (problem, set_problem) = create_signal(None);
        Self {
            tasks,
            shell: Rc::new(shell),
            builds: Rc::new(RefCell::new(HashMap::new())),
            downloaded,
            set_downloaded,
            work,
            set_work,
            problem,
            set_problem,
        }
    }

    pub(crate) fn start(&self) {
        self.tasks.background(|tasks| {
            let directory = android::files(tasks).join(BUILD);
            tasks::Event::Phone(Event::Found(builds::downloaded(&directory)))
        });
    }

    pub(crate) fn holds(&self, slot: Slot) -> bool {
        self.downloaded.with(|downloaded| {
            downloaded
                .as_ref()
                .is_some_and(|downloaded| downloaded.slot == slot)
        })
    }

    pub(crate) fn runs_here(&self, build: &Build) -> bool {
        build.shell == *self.shell
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
        if self.work.with(Option::is_some) {
            return;
        }
        self.set_problem.set(None);
        self.set_work.set(Some(Work {
            slot,
            stage: Stage::Downloading { done: 0, total: 0 },
        }));
        let shell = self.shell.to_string();
        self.tasks.background(move |tasks| {
            android::stop();
            let files = android::files(tasks);
            let result = fetch_build(slot, &commit).and_then(|build| {
                let build = build.ok_or_else(|| "be3-ci has no Android build of it".to_owned())?;
                if build.shell != shell {
                    return Err(
                        "This build changes the app's Java, so it runs only in the launcher built with it."
                            .to_owned(),
                    );
                }
                let mut reported = 0;
                let mut progress = |done: u64, total: u64| {
                    if done == total || done >= reported + PROGRESS_STEP {
                        reported = done;
                        tasks.send(tasks::Event::Phone(Event::Progress(slot, done, total)));
                    }
                };
                builds::sync(
                    &files.join(BUILD),
                    slot,
                    &build,
                    &mut android::Http,
                    &mut progress,
                )?;
                if fresh {
                    remove_data(&files.join(DATA))?;
                }
                Ok(())
            });
            let downloaded = builds::downloaded(&files.join(BUILD));
            tasks::Event::Phone(Event::Synced(slot, result, downloaded))
        });
    }

    pub(crate) fn install(&self, slot: Slot, commit: String) {
        if self.work.with(Option::is_some) {
            return;
        }
        self.set_problem.set(None);
        self.set_work.set(Some(Work {
            slot,
            stage: Stage::Installing,
        }));
        self.tasks.background(move |tasks| {
            let apk = android::files(tasks).join(LAUNCHER);
            let result = fetch_build(slot, &commit).and_then(|build| {
                let build = build.ok_or_else(|| "be3-ci has no Android build of it".to_owned())?;
                builds::fetch_launcher(&apk, slot, &build, &mut android::Http)?;
                android::install(&apk)
            });
            match result {
                Ok(()) => tasks::Event::Phone(Event::Confirming),
                Err(error) => tasks::Event::Phone(Event::Installed(Err(error))),
            }
        });
    }

    pub(crate) fn stop(&self) {
        android::stop();
    }

    pub(crate) fn receive(&self, event: Event) {
        match event {
            Event::Found(downloaded) => self.set_downloaded.set(downloaded),
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
                if let Some(Work {
                    stage: Stage::Downloading { .. },
                    ..
                }) = self.work.get_untracked()
                {
                    self.set_work.set(Some(Work {
                        slot,
                        stage: Stage::Downloading { done, total },
                    }));
                }
            }
            Event::Synced(slot, result, downloaded) => {
                self.set_work.set(None);
                self.set_downloaded.set(downloaded);
                match result {
                    Ok(()) => {
                        let files = android::files(&self.tasks);
                        if let Err(error) = android::launch(&files.join(BUILD), &files.join(DATA)) {
                            self.set_problem.set(Some((slot, error)));
                        }
                    }
                    Err(error) => self.set_problem.set(Some((slot, error))),
                }
            }
            Event::Confirming => {}
            Event::Installed(result) => {
                let slot = self.work.get_untracked().map(|work| work.slot);
                self.set_work.set(None);
                if let (Err(error), Some(slot)) = (result, slot) {
                    self.set_problem.set(Some((slot, error)));
                }
            }
        }
    }
}

fn fetch_build(slot: Slot, commit: &str) -> Result<Option<Build>, String> {
    match android::get(&slot.manifest_url(commit))? {
        Some(document) => Build::parse(&document).map(Some),
        None => Ok(None),
    }
}

fn remove_data(data: &std::path::Path) -> Result<(), String> {
    match std::fs::remove_dir_all(data) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("Could not clear the build's data: {error}"))
        }
        _ => Ok(()),
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
    let runnable = create_memo(clone!(build phone -> move || match build.get() {
        Loaded::Ready(Some(build)) => Some(phone.runs_here(&build)),
        _ => None,
    }));
    let busy = create_memo(clone!(phone -> move || phone.work.with(Option::is_some)));
    let disabled =
        create_memo(clone!(runnable busy -> move || busy.get() || runnable.get().is_none()));
    let menu_disabled = busy.clone();
    let label = create_memo(clone!(runnable -> move || match runnable.get() {
        Some(false) => "Install its launcher",
        _ => "Run",
    }.to_owned()));
    let glyph = create_memo(clone!(runnable -> move || match runnable.get() {
        Some(false) => ICON_SYSTEM_UPDATE,
        _ => ICON_PLAY_ARROW,
    }.to_owned()));
    let primary = clone!(phone pull_request runnable -> move || {
        let head = pull_request.get_untracked().head_sha;
        match runnable.get_untracked() {
            Some(true) => phone.run(slot, head, false),
            Some(false) => phone.install(slot, head),
            None => {}
        }
    });
    let pick = clone!(phone pull_request -> move |path: Vec<usize>| {
        let head = pull_request.get_untracked().head_sha;
        match path.as_slice() {
            [0] => phone.run(slot, head, true),
            [1] => phone.install(slot, head),
            [2] => phone.stop(),
            _ => {}
        }
    });
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=1.0>
            <Button label glyph variant=ButtonVariant::Primary disabled on_click={primary} />
            <MenuButton
                label="More ways to run it"
                variant=ButtonVariant::Primary
                icon_only=true
                disabled={menu_disabled}
                items={view! {
                    <MenuItem label="Run with fresh data" />
                    <MenuItem label="Install its launcher" />
                    <MenuItem label="Stop the running build" />
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
    let status = create_memo(clone!(build phone pull_request -> move || {
        let pull_request = pull_request.get();
        match build.get() {
            Loaded::Loading => "Looking for its Android build".to_owned(),
            Loaded::Failed(error) => error,
            Loaded::Ready(None) if !pull_request.same_repository => {
                "It comes from a fork, so CI does not publish an Android build of it.".to_owned()
            }
            Loaded::Ready(None) => {
                "CI has not published an Android build of it yet.".to_owned()
            }
            Loaded::Ready(Some(build)) => {
                let mut status = if build.commit == pull_request.head_sha {
                    format!("The Android build of {} is {}.", short(&build.commit), megabytes(build.size()))
                } else {
                    format!(
                        "CI has not published {} yet, so this runs {}.",
                        short(&pull_request.head_sha),
                        short(&build.commit)
                    )
                };
                if !phone.runs_here(&build) {
                    status.push_str(" It changes the app's Java, so it runs only in the launcher built with it.");
                }
                status
            }
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
pub(crate) fn LauncherMenu(model: Model) -> NodeId {
    let phone = model.phone.clone();
    let busy = create_memo(clone!(phone -> move || phone.work.with(Option::is_some)));
    let pick = move |path: Vec<usize>| match path.as_slice() {
        [0] => phone.run(Slot::Main, now_key(), false),
        [1] => phone.run(Slot::Main, now_key(), true),
        [2] => phone.install(Slot::Main, now_key()),
        [3] => phone.stop(),
        _ => {}
    };
    view! {
        <MenuButton
            label="Main and the launcher"
            glyph=ICON_MORE_VERT
            variant=ButtonVariant::Ghost
            icon_only=true
            arrow=false
            disabled={busy}
            items={view! {
                <MenuItem label="Run main" />
                <MenuItem label="Run main with fresh data" />
                <MenuItem label="Install main's launcher" />
                <MenuItem label="Stop the running build" />
            }}
            on_select={pick}
        />
    }
}

#[component]
pub(crate) fn MainNotes(model: Model) -> NodeId {
    view! {
        <SlotNotes model slot={Slot::Main} />
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
        Some(Work { stage: Stage::Downloading { total: 0, .. }, .. }) => "Getting the build ready".to_owned(),
        Some(Work { stage: Stage::Downloading { done, total }, .. }) => {
            format!("Downloading {} of {}", megabytes(done), megabytes(total))
        }
        Some(Work { stage: Stage::Installing, .. }) => "Installing the launcher".to_owned(),
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
