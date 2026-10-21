#[cfg(not(target_os = "android"))]
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;

use beui::Image;
use beui::reactive::*;
#[cfg(not(target_os = "android"))]
use beui::unstyled::DockingLayout;

#[cfg(not(target_os = "android"))]
use crate::github::branch_deleted;
use crate::github::{Entry, Filter, PullRequest, timeline_images};
#[cfg(not(target_os = "android"))]
use crate::pane::Pane;
#[cfg(target_os = "android")]
use crate::phone::Phone;
#[cfg(not(target_os = "android"))]
use crate::targets::{Action, Target};
use crate::tasks::{Event, Tasks, open_url};

#[cfg(not(target_os = "android"))]
const BOLD: &str = "\x1b[1m";
#[cfg(not(target_os = "android"))]
const GREEN: &str = "\x1b[32m";
#[cfg(not(target_os = "android"))]
const RED: &str = "\x1b[31m";
#[cfg(not(target_os = "android"))]
const RESET: &str = "\x1b[0m";

#[cfg(not(target_os = "android"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Common {
    pub(crate) title: &'static str,
    pub(crate) args: &'static str,
}

#[cfg(not(target_os = "android"))]
pub(crate) const COMMON: [Common; 5] = [
    Common {
        title: "the app",
        args: "buck run //crates/block-app:native",
    },
    Common {
        title: "verify",
        args: "verify",
    },
    Common {
        title: "the web app",
        args: "buck run //crates/block-app:web",
    },
    Common {
        title: "on Android",
        args: "buck run //crates/block-app:android -- --install",
    },
    Common {
        title: "beui's demo",
        args: "buck run //crates/beui-demo:native",
    },
];

#[cfg(not(target_os = "android"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Tab {
    PullRequest(u64),
    Targets,
    Log,
}

#[cfg(not(target_os = "android"))]
fn words(args: &str) -> Vec<String> {
    args.split_whitespace().map(str::to_owned).collect()
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Loaded<T> {
    Loading,
    Ready(T),
    Failed(String),
}

pub(crate) type Cache<K, T> = Stored<RefCell<HashMap<K, Signal<Loaded<T>>>>>;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Viewer {
    pub(crate) images: Vec<String>,
    pub(crate) index: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct Model {
    tasks: Stored<Tasks>,
    owner: Stored<Option<ScopeContext>>,
    pub(crate) repository: Signal<Loaded<String>>,
    pub(crate) filter: Signal<Filter>,
    pub(crate) listing: Signal<Loaded<()>>,
    pub(crate) pull_requests: KeyedStore<u64, PullRequest>,
    pub(crate) query: Signal<String>,
    pub(crate) opened: Signal<Vec<PullRequest>>,
    pub(crate) selection: Selector<Option<u64>>,
    timelines: Cache<u64, Vec<Entry>>,
    images: Cache<String, Image>,
    #[cfg(not(target_os = "android"))]
    pub(crate) head: Signal<String>,
    #[cfg(not(target_os = "android"))]
    pub(crate) head_summary: Signal<String>,
    #[cfg(not(target_os = "android"))]
    pub(crate) common: Signal<usize>,
    pub(crate) viewer: Signal<Option<Viewer>>,
    #[cfg(not(target_os = "android"))]
    pub(crate) layout: DockingLayout<Tab>,
    #[cfg(not(target_os = "android"))]
    stopped: Stored<Cell<bool>>,
    #[cfg(not(target_os = "android"))]
    pub(crate) targets: Signal<Option<Loaded<Vec<Target>>>>,
    #[cfg(not(target_os = "android"))]
    pub(crate) target_query: Signal<String>,
    #[cfg(not(target_os = "android"))]
    pub(crate) running: Signal<bool>,
    #[cfg(not(target_os = "android"))]
    pub(crate) pane: Pane,
    #[cfg(target_os = "android")]
    pub(crate) phone: Phone,
}

impl Model {
    #[cfg(target_os = "android")]
    pub(crate) fn new(tasks: Tasks, phone: Phone) -> Self {
        Self::shared(tasks, phone)
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn new(tasks: Tasks, pane: Pane) -> Self {
        Self::shared(tasks, pane)
    }

    fn shared(
        tasks: Tasks,
        #[cfg(not(target_os = "android"))] pane: Pane,
        #[cfg(target_os = "android")] phone: Phone,
    ) -> Self {
        let repository = create_signal(Loaded::Loading);
        let filter = create_signal(Filter::Open);
        let listing = create_signal(Loaded::Loading);
        let query = create_signal(String::new());
        let opened = create_signal(Vec::<PullRequest>::new());
        #[cfg(not(target_os = "android"))]
        let layout = DockingLayout::new();
        #[cfg(not(target_os = "android"))]
        let selection = create_selector(move || shown_pull_request(layout));
        #[cfg(target_os = "android")]
        let selection = create_selector(move || {
            opened.with(|opened| opened.first().map(|pull_request| pull_request.number))
        });
        #[cfg(not(target_os = "android"))]
        let head = create_signal(String::new());
        #[cfg(not(target_os = "android"))]
        let head_summary = create_signal(String::new());
        #[cfg(not(target_os = "android"))]
        let common = create_signal(0usize);
        let viewer = create_signal(None);
        #[cfg(not(target_os = "android"))]
        let targets = create_signal(None);
        #[cfg(not(target_os = "android"))]
        let target_query = create_signal(String::new());
        #[cfg(not(target_os = "android"))]
        let running = create_signal(false);
        let model = Self {
            tasks: store(tasks),
            owner: store(owner_scope()),
            repository,
            filter,
            listing,
            pull_requests: KeyedStore::new(),
            query,
            opened,
            selection,
            timelines: store(RefCell::default()),
            images: store(RefCell::default()),
            #[cfg(not(target_os = "android"))]
            head,
            #[cfg(not(target_os = "android"))]
            head_summary,
            #[cfg(not(target_os = "android"))]
            common,
            viewer,
            #[cfg(not(target_os = "android"))]
            layout,
            #[cfg(not(target_os = "android"))]
            stopped: store(Cell::default()),
            #[cfg(not(target_os = "android"))]
            targets,
            #[cfg(not(target_os = "android"))]
            target_query,
            #[cfg(not(target_os = "android"))]
            running,
            #[cfg(not(target_os = "android"))]
            pane,
            #[cfg(target_os = "android")]
            phone,
        };
        #[cfg(not(target_os = "android"))]
        model.list_targets_when_shown();
        model
    }

    #[cfg(not(target_os = "android"))]
    fn list_targets_when_shown(&self) {
        let model = *self;
        create_effect(move || {
            if model.showing(Tab::Targets) && model.targets.get_untracked().is_none() {
                model.refresh_targets();
            }
        });
    }

    pub(crate) fn start(&self) {
        #[cfg(not(target_os = "android"))]
        self.pane
            .write_line("Output from ./scripts/switch and ./scripts/buck shows here.");
        self.tasks.value().connect();
        self.tasks.value().list(Filter::Open);
        #[cfg(not(target_os = "android"))]
        self.tasks.value().read_head();
        #[cfg(target_os = "android")]
        self.phone.start();
    }

    pub(crate) fn show(&self, filter: Filter) {
        if self.filter.get_untracked() == filter {
            return;
        }
        self.filter.set(filter);
        self.refresh_list();
    }

    pub(crate) fn refresh_list(&self) {
        self.listing.set(Loaded::Loading);
        self.tasks.value().list(self.filter.get_untracked());
        #[cfg(target_os = "android")]
        self.phone.refresh_main();
    }

    #[cfg(target_os = "android")]
    pub(crate) fn select(&self, number: u64, _beside: bool) {
        if let Some(pull_request) = self.pull_requests.try_get(&number) {
            self.opened.set(vec![pull_request.get_untracked()]);
        }
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn select(&self, number: u64, beside: bool) {
        let Some(pull_request) = self.pull_requests.try_get(&number) else {
            return;
        };
        let pull_request = pull_request.get_untracked();
        if self.pull_request(number).is_some() {
            self.layout.show(&Tab::PullRequest(number));
            return;
        }
        let shown = untrack(|| shown_pull_request(self.layout)).filter(|_| !beside);
        match shown {
            Some(shown)
                if self
                    .layout
                    .replace(&Tab::PullRequest(shown), Tab::PullRequest(number)) =>
            {
                self.opened.update(|opened| {
                    for held in opened.iter_mut().filter(|held| held.number == shown) {
                        *held = pull_request.clone();
                    }
                });
            }
            _ => self.opened.update(|opened| opened.push(pull_request)),
        }
    }

    #[cfg(target_os = "android")]
    pub(crate) fn deselect(&self) {
        self.opened.set(Vec::new());
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn close(&self, number: u64) {
        self.opened
            .update(|opened| opened.retain(|held| held.number != number));
    }

    #[cfg(not(target_os = "android"))]
    fn pull_request(&self, number: u64) -> Option<PullRequest> {
        self.opened.with_untracked(|opened| {
            opened
                .iter()
                .find(|pull_request| pull_request.number == number)
                .cloned()
        })
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn current(&self, pull_request: &PullRequest) -> bool {
        let head = self.head.get();
        !head.is_empty() && head == pull_request.head_sha
    }

    #[cfg(target_os = "android")]
    pub(crate) fn current(&self, pull_request: &PullRequest) -> bool {
        self.phone
            .holds(crate::builds::Slot::PullRequest(pull_request.number))
    }

    fn owned<T>(&self, make: impl FnOnce() -> T) -> T {
        match self.owner.value().as_ref() {
            Some(owner) => owner.run(make),
            None => make(),
        }
    }

    pub(crate) fn timeline(&self, pull_request: &PullRequest) -> ReadSignal<Loaded<Vec<Entry>>> {
        if let Some(slot) = self.timelines.value().borrow().get(&pull_request.number) {
            return slot.read_only();
        }
        let loaded = self.owned(|| create_signal(Loaded::Loading));
        self.timelines
            .value()
            .borrow_mut()
            .insert(pull_request.number, loaded);
        self.tasks.value().timeline(pull_request.clone());
        loaded.read_only()
    }

    pub(crate) fn refresh_timeline(&self, pull_request: &PullRequest) {
        let loaded = self
            .timelines
            .value()
            .borrow()
            .get(&pull_request.number)
            .copied();
        if let Some(loaded) = loaded {
            loaded.set(Loaded::Loading);
        }
        self.tasks.value().timeline(pull_request.clone());
        #[cfg(target_os = "android")]
        self.phone.refresh(
            crate::builds::Slot::PullRequest(pull_request.number),
            &pull_request.head_sha,
        );
    }

    pub(crate) fn view_image(&self, number: u64, url: &str) {
        let images = Some(number)
            .and_then(|number| {
                let timeline = *self.timelines.value().borrow().get(&number)?;
                match timeline.get_untracked() {
                    Loaded::Ready(entries) => Some(timeline_images(&entries)),
                    _ => None,
                }
            })
            .unwrap_or_default();
        let (images, index) = match images.iter().position(|candidate| candidate == url) {
            Some(index) => (images, index),
            None => (vec![url.to_owned()], 0),
        };
        self.viewer.set(Some(Viewer { images, index }));
    }

    pub(crate) fn step_image(&self, step: isize) {
        self.viewer.update(|viewer| {
            if let Some(viewer) = viewer {
                let last = viewer.images.len().saturating_sub(1) as isize;
                viewer.index = (viewer.index as isize + step).clamp(0, last) as usize;
            }
        });
    }

    pub(crate) fn close_image(&self) {
        self.viewer.set(None);
    }

    pub(crate) fn image(&self, url: &str) -> ReadSignal<Loaded<Image>> {
        if let Some(slot) = self.images.value().borrow().get(url) {
            return slot.read_only();
        }
        let loaded = self.owned(|| create_signal(Loaded::Loading));
        self.images
            .value()
            .borrow_mut()
            .insert(url.to_owned(), loaded);
        self.tasks.value().image(url.to_owned());
        loaded.read_only()
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn show_tab(&self, tab: Tab) {
        self.layout.show(&tab);
    }

    #[cfg(not(target_os = "android"))]
    fn showing(&self, tab: Tab) -> bool {
        let Some(tab) = self.layout.tab_of(&tab) else {
            return false;
        };
        self.layout.state().with(|state| {
            state
                .find(tab)
                .is_some_and(|position| state.active_tab(position.leaf) == Some(tab))
        })
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn refresh_targets(&self) {
        self.targets.set(Some(Loaded::Loading));
        self.tasks.value().targets();
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn run(&self, args: Vec<String>) {
        if args.is_empty() || self.running.get_untracked() {
            return;
        }
        self.running.set(true);
        self.tasks.value().run("buck", args);
    }

    #[cfg(not(target_os = "android"))]
    fn run_script(&self, mut words: Vec<String>) {
        if words.is_empty() || self.running.get_untracked() {
            return;
        }
        let script = words.remove(0);
        self.running.set(true);
        self.tasks.value().run(&script, words);
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn run_target(&self, target: &Target, action: Action) {
        self.run(vec![action.verb().to_owned(), target.label.clone()]);
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn check_out(&self, pull_request: &PullRequest, then_run: Option<usize>) {
        self.switch(&pull_request.branch, then_run);
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn switch(&self, branch: &str, then_run: Option<usize>) {
        if self.running.get_untracked() {
            return;
        }
        let mut args = vec![branch.to_owned()];
        if let Some(common) =
            then_run.and_then(|index| COMMON.get(index).map(|common| (index, common)))
        {
            self.common.set(common.0);
            args.extend(words(common.1.args));
        }
        self.running.set(true);
        self.tasks.value().run("switch", args);
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn run_again(&self) {
        let common = self.common.get_untracked();
        let shown = untrack(|| shown_pull_request(self.layout))
            .and_then(|number| self.pull_request(number));
        match shown {
            Some(pull_request) if self.can_check_out(&pull_request) => {
                self.check_out(&pull_request, Some(common));
            }
            _ => {
                if let Some(common) = COMMON.get(common) {
                    self.run_script(words(common.args));
                }
            }
        }
    }

    #[cfg(not(target_os = "android"))]
    fn can_check_out(&self, pull_request: &PullRequest) -> bool {
        let deleted = self
            .timelines
            .value()
            .borrow()
            .get(&pull_request.number)
            .is_some_and(|timeline| match timeline.get_untracked() {
                Loaded::Ready(entries) => branch_deleted(&entries),
                _ => false,
            });
        pull_request.same_repository && !deleted
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn stop(&self) {
        self.stopped.value().set(true);
        self.tasks.value().stop();
    }

    pub(crate) fn open(&self, url: &str) {
        open_url(url);
    }

    pub(crate) fn receive(&self, event: Event) {
        match event {
            Event::Connected(repository) => self.repository.set(match repository {
                Ok(name) => Loaded::Ready(name),
                Err(error) => Loaded::Failed(error),
            }),
            Event::PullRequests(filter, listed) => {
                if filter != self.filter.get_untracked() {
                    return;
                }
                match listed {
                    Ok(listed) => {
                        let stale = self.opened.with_untracked(|opened| {
                            opened.iter().any(|held| {
                                listed.iter().any(|pull_request| {
                                    pull_request.number == held.number && pull_request != held
                                })
                            })
                        });
                        if stale {
                            self.opened.update(|opened| {
                                for held in opened.iter_mut() {
                                    if let Some(refreshed) = listed
                                        .iter()
                                        .find(|pull_request| pull_request.number == held.number)
                                    {
                                        *held = refreshed.clone();
                                    }
                                }
                            });
                        }
                        self.pull_requests.reconcile_owned(
                            listed
                                .into_iter()
                                .map(|pull_request| (pull_request.number, pull_request)),
                        );
                        self.listing.set(Loaded::Ready(()));
                    }
                    Err(error) => self.listing.set(Loaded::Failed(error)),
                }
            }
            Event::Timeline(number, timeline) => {
                let loaded = self.timelines.value().borrow().get(&number).copied();
                if let Some(loaded) = loaded {
                    loaded.set(match timeline {
                        Ok(entries) => Loaded::Ready(entries),
                        Err(error) => Loaded::Failed(error),
                    });
                }
            }
            Event::Image(url, pixels) => {
                let loaded = self.images.value().borrow().get(&url).copied();
                if let Some(loaded) = loaded {
                    loaded.set(match pixels {
                        Ok(pixels) => Loaded::Ready(Image::from_rgba(
                            pixels.width,
                            pixels.height,
                            pixels.rgba,
                        )),
                        Err(error) => Loaded::Failed(error),
                    });
                }
            }
            #[cfg(not(target_os = "android"))]
            Event::Head { sha, summary } => {
                let previous = self.head.get_untracked();
                if !previous.is_empty() && previous != sha {
                    self.targets.set(None);
                    if untrack(|| self.showing(Tab::Targets)) {
                        self.refresh_targets();
                    }
                }
                self.head.set(sha);
                self.head_summary.set(summary);
            }
            #[cfg(not(target_os = "android"))]
            Event::Started(description) => {
                self.running.set(true);
                self.stopped.value().set(false);
                self.pane
                    .write_line(&format!("{BOLD}$ {description}{RESET}"));
            }
            #[cfg(not(target_os = "android"))]
            Event::Targets(targets) => self.targets.set(Some(match targets {
                Ok(targets) => Loaded::Ready(targets),
                Err(error) => Loaded::Failed(error),
            })),
            #[cfg(not(target_os = "android"))]
            Event::Output(bytes) => self.pane.write(&bytes),
            #[cfg(not(target_os = "android"))]
            Event::Finished { summary, success } => {
                let color = if success { GREEN } else { RED };
                self.pane.write_line(&format!("{color}{summary}{RESET}\n"));
                self.running.set(false);
                if !success && !self.stopped.value().replace(false) {
                    self.layout.show(&Tab::Log);
                }
            }
            #[cfg(target_os = "android")]
            Event::Phone(event) => self.phone.receive(event),
        }
    }
}

#[cfg(not(target_os = "android"))]
fn shown_pull_request(layout: DockingLayout<Tab>) -> Option<u64> {
    layout.state().with(|state| {
        state
            .recent_tabs()
            .into_iter()
            .find_map(|tab| match layout.key_of(tab) {
                Some(Tab::PullRequest(number)) => Some(number),
                _ => None,
            })
    })
}
