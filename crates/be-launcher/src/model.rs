use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use beui::Image;
use beui::reactive::{
    KeyedStore, ReadSignal, Selector, WriteSignal, clone, create_selector, create_signal,
};

use crate::github::{Entry, Filter, PullRequest, branch_deleted, timeline_images};
use crate::pane::Pane;
use crate::targets::{Action, Target};
use crate::tasks::{Event, Tasks, open_url};

const BOLD: &str = "\x1b[1m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Common {
    pub(crate) title: &'static str,
    pub(crate) args: &'static str,
}

pub(crate) const COMMON: [Common; 7] = [
    Common {
        title: "the app",
        args: "run //crates/block-app:app",
    },
    Common {
        title: "verify",
        args: "run //:verify",
    },
    Common {
        title: "the web app",
        args: "run //crates/block-app:web-serve",
    },
    Common {
        title: "on Android",
        args: "run //crates/block-app:android -- --install",
    },
    Common {
        title: "beui's demo",
        args: "run //crates/beui:demo-example",
    },
    Common {
        title: "beui's docking demo",
        args: "run //crates/beui:dock-example",
    },
    Common {
        title: "be-compositor",
        args: "run //crates/be-compositor:be-compositor-bin",
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tab {
    PullRequest,
    Targets,
    Log,
}

impl Tab {
    pub(crate) const ALL: [Tab; 3] = [Tab::PullRequest, Tab::Targets, Tab::Log];
}

fn words(args: &str) -> Vec<String> {
    args.split_whitespace().map(str::to_owned).collect()
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Loaded<T> {
    Loading,
    Ready(T),
    Failed(String),
}

type Slot<T> = (ReadSignal<T>, WriteSignal<T>);
type Cache<K, T> = Rc<RefCell<HashMap<K, Slot<Loaded<T>>>>>;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Viewer {
    pub(crate) images: Vec<String>,
    pub(crate) index: usize,
}

#[derive(Clone)]
pub(crate) struct Model {
    tasks: Tasks,
    pub(crate) repository: ReadSignal<Loaded<String>>,
    set_repository: WriteSignal<Loaded<String>>,
    pub(crate) filter: ReadSignal<Filter>,
    set_filter: WriteSignal<Filter>,
    pub(crate) listing: ReadSignal<Loaded<()>>,
    set_listing: WriteSignal<Loaded<()>>,
    pub(crate) pull_requests: KeyedStore<u64, PullRequest>,
    pub(crate) query: ReadSignal<String>,
    pub(crate) set_query: WriteSignal<String>,
    pub(crate) selected: ReadSignal<Option<PullRequest>>,
    set_selected: WriteSignal<Option<PullRequest>>,
    pub(crate) selection: Selector<Option<u64>>,
    timelines: Cache<u64, Vec<Entry>>,
    images: Cache<String, Image>,
    pub(crate) head: ReadSignal<String>,
    set_head: WriteSignal<String>,
    pub(crate) head_summary: ReadSignal<String>,
    set_head_summary: WriteSignal<String>,
    pub(crate) common: ReadSignal<usize>,
    set_common: WriteSignal<usize>,
    pub(crate) viewer: ReadSignal<Option<Viewer>>,
    set_viewer: WriteSignal<Option<Viewer>>,
    pub(crate) tab: ReadSignal<Tab>,
    set_tab: WriteSignal<Tab>,
    stopped: Rc<Cell<bool>>,
    pub(crate) targets: ReadSignal<Option<Loaded<Vec<Target>>>>,
    set_targets: WriteSignal<Option<Loaded<Vec<Target>>>>,
    pub(crate) target_query: ReadSignal<String>,
    pub(crate) set_target_query: WriteSignal<String>,
    pub(crate) running: ReadSignal<bool>,
    set_running: WriteSignal<bool>,
    pub(crate) pane: Pane,
}

impl Model {
    pub(crate) fn new(tasks: Tasks, pane: Pane) -> Self {
        let (repository, set_repository) = create_signal(Loaded::Loading);
        let (filter, set_filter) = create_signal(Filter::Open);
        let (listing, set_listing) = create_signal(Loaded::Loading);
        let (query, set_query) = create_signal(String::new());
        let (selected, set_selected) = create_signal(None::<PullRequest>);
        let selection = create_selector(clone!(selected -> move || {
            selected.with(|selected| selected.as_ref().map(|pull_request| pull_request.number))
        }));
        let (head, set_head) = create_signal(String::new());
        let (head_summary, set_head_summary) = create_signal(String::new());
        let (common, set_common) = create_signal(0usize);
        let (tab, set_tab) = create_signal(Tab::PullRequest);
        let (viewer, set_viewer) = create_signal(None);
        let (targets, set_targets) = create_signal(None);
        let (target_query, set_target_query) = create_signal(String::new());
        let (running, set_running) = create_signal(false);
        Self {
            tasks,
            repository,
            set_repository,
            filter,
            set_filter,
            listing,
            set_listing,
            pull_requests: KeyedStore::new(),
            query,
            set_query,
            selected,
            set_selected,
            selection,
            timelines: Rc::default(),
            images: Rc::default(),
            head,
            set_head,
            head_summary,
            set_head_summary,
            common,
            set_common,
            viewer,
            set_viewer,
            tab,
            set_tab,
            stopped: Rc::default(),
            targets,
            set_targets,
            target_query,
            set_target_query,
            running,
            set_running,
            pane,
        }
    }

    pub(crate) fn start(&self) {
        self.pane
            .write_line("Output from ./scripts/switch and ./scripts/buck shows here.");
        self.tasks.connect();
        self.tasks.list(Filter::Open);
        self.tasks.read_head();
    }

    pub(crate) fn show(&self, filter: Filter) {
        if self.filter.get_untracked() == filter {
            return;
        }
        self.set_filter.set(filter);
        self.refresh_list();
    }

    pub(crate) fn refresh_list(&self) {
        self.set_listing.set(Loaded::Loading);
        self.tasks.list(self.filter.get_untracked());
    }

    pub(crate) fn select(&self, number: u64) {
        if let Some(pull_request) = self.pull_requests.try_get(&number) {
            self.set_selected.set(Some(pull_request.get_untracked()));
        }
        self.set_tab.set(Tab::PullRequest);
    }

    pub(crate) fn timeline(&self, pull_request: &PullRequest) -> ReadSignal<Loaded<Vec<Entry>>> {
        if let Some((read, _)) = self.timelines.borrow().get(&pull_request.number) {
            return read.clone();
        }
        let (read, write) = create_signal(Loaded::Loading);
        self.timelines
            .borrow_mut()
            .insert(pull_request.number, (read.clone(), write));
        self.tasks.timeline(pull_request.clone());
        read
    }

    pub(crate) fn refresh_timeline(&self, pull_request: &PullRequest) {
        let write = self
            .timelines
            .borrow()
            .get(&pull_request.number)
            .map(|(_, write)| write.clone());
        if let Some(write) = write {
            write.set(Loaded::Loading);
        }
        self.tasks.timeline(pull_request.clone());
    }

    pub(crate) fn view_image(&self, url: &str) {
        let images = self
            .selected
            .get_untracked()
            .and_then(|pull_request| {
                let timeline = self.timelines.borrow().get(&pull_request.number)?.0.clone();
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
        self.set_viewer.set(Some(Viewer { images, index }));
    }

    pub(crate) fn step_image(&self, step: isize) {
        self.set_viewer.update(|viewer| {
            if let Some(viewer) = viewer {
                let last = viewer.images.len().saturating_sub(1) as isize;
                viewer.index = (viewer.index as isize + step).clamp(0, last) as usize;
            }
        });
    }

    pub(crate) fn close_image(&self) {
        self.set_viewer.set(None);
    }

    pub(crate) fn image(&self, url: &str) -> ReadSignal<Loaded<Image>> {
        if let Some((read, _)) = self.images.borrow().get(url) {
            return read.clone();
        }
        let (read, write) = create_signal(Loaded::Loading);
        self.images
            .borrow_mut()
            .insert(url.to_owned(), (read.clone(), write));
        self.tasks.image(url.to_owned());
        read
    }

    pub(crate) fn show_tab(&self, tab: Tab) {
        self.set_tab.set(tab);
        if tab == Tab::Targets && self.targets.get_untracked().is_none() {
            self.refresh_targets();
        }
    }

    pub(crate) fn refresh_targets(&self) {
        self.set_targets.set(Some(Loaded::Loading));
        self.tasks.targets();
    }

    pub(crate) fn run(&self, args: Vec<String>) {
        if args.is_empty() || self.running.get_untracked() {
            return;
        }
        self.set_running.set(true);
        self.tasks.run("buck", args);
    }

    pub(crate) fn run_target(&self, target: &Target, action: Action) {
        self.run(vec![action.verb().to_owned(), target.label.clone()]);
    }

    pub(crate) fn check_out(&self, pull_request: &PullRequest, then_run: Option<usize>) {
        if self.running.get_untracked() {
            return;
        }
        let mut args = vec![pull_request.branch.clone()];
        if let Some(common) =
            then_run.and_then(|index| COMMON.get(index).map(|common| (index, common)))
        {
            self.set_common.set(common.0);
            args.extend(words(common.1.args));
        }
        self.set_running.set(true);
        self.tasks.run("switch", args);
    }

    pub(crate) fn run_again(&self) {
        let common = self.common.get_untracked();
        match self.selected.get_untracked() {
            Some(pull_request) if self.can_check_out(&pull_request) => {
                self.check_out(&pull_request, Some(common));
            }
            _ => {
                if let Some(common) = COMMON.get(common) {
                    self.run(words(common.args));
                }
            }
        }
    }

    fn can_check_out(&self, pull_request: &PullRequest) -> bool {
        let deleted = self
            .timelines
            .borrow()
            .get(&pull_request.number)
            .is_some_and(|(timeline, _)| match timeline.get_untracked() {
                Loaded::Ready(entries) => branch_deleted(&entries),
                _ => false,
            });
        pull_request.same_repository && !deleted
    }

    pub(crate) fn stop(&self) {
        self.stopped.set(true);
        self.tasks.stop();
    }

    pub(crate) fn open(&self, url: &str) {
        open_url(url);
    }

    pub(crate) fn receive(&self, event: Event) {
        match event {
            Event::Connected(repository) => self.set_repository.set(match repository {
                Ok(name) => Loaded::Ready(name),
                Err(error) => Loaded::Failed(error),
            }),
            Event::PullRequests(filter, listed) => {
                if filter != self.filter.get_untracked() {
                    return;
                }
                match listed {
                    Ok(listed) => {
                        let refreshed = self.selected.get_untracked().and_then(|selected| {
                            listed
                                .iter()
                                .find(|pull_request| pull_request.number == selected.number)
                                .cloned()
                        });
                        if let Some(refreshed) = refreshed {
                            self.set_selected.set(Some(refreshed));
                        }
                        self.pull_requests.reconcile_owned(
                            listed
                                .into_iter()
                                .map(|pull_request| (pull_request.number, pull_request)),
                        );
                        self.set_listing.set(Loaded::Ready(()));
                    }
                    Err(error) => self.set_listing.set(Loaded::Failed(error)),
                }
            }
            Event::Timeline(number, timeline) => {
                let write = self
                    .timelines
                    .borrow()
                    .get(&number)
                    .map(|(_, write)| write.clone());
                if let Some(write) = write {
                    write.set(match timeline {
                        Ok(entries) => Loaded::Ready(entries),
                        Err(error) => Loaded::Failed(error),
                    });
                }
            }
            Event::Image(url, pixels) => {
                let write = self
                    .images
                    .borrow()
                    .get(&url)
                    .map(|(_, write)| write.clone());
                if let Some(write) = write {
                    write.set(match pixels {
                        Ok(pixels) => Loaded::Ready(Image::from_rgba(
                            pixels.width,
                            pixels.height,
                            pixels.rgba,
                        )),
                        Err(error) => Loaded::Failed(error),
                    });
                }
            }
            Event::Head { sha, summary } => {
                let previous = self.head.get_untracked();
                if !previous.is_empty() && previous != sha {
                    self.set_targets.set(None);
                    if self.tab.get_untracked() == Tab::Targets {
                        self.refresh_targets();
                    }
                }
                self.set_head.set(sha);
                self.set_head_summary.set(summary);
            }
            Event::Started(description) => {
                self.set_running.set(true);
                self.stopped.set(false);
                self.pane
                    .write_line(&format!("{BOLD}$ {description}{RESET}"));
            }
            Event::Targets(targets) => self.set_targets.set(Some(match targets {
                Ok(targets) => Loaded::Ready(targets),
                Err(error) => Loaded::Failed(error),
            })),
            Event::Output(bytes) => self.pane.write(&bytes),
            Event::Finished { summary, success } => {
                let color = if success { GREEN } else { RED };
                self.pane.write_line(&format!("{color}{summary}{RESET}\n"));
                self.set_running.set(false);
                if !success && !self.stopped.replace(false) {
                    self.set_tab.set(Tab::Log);
                }
            }
        }
    }
}
