#[cfg(not(target_os = "android"))]
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use beui::Image;
use beui::reactive::{
    KeyedStore, ReadSignal, Selector, WriteSignal, clone, create_selector, create_signal,
};

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

#[cfg(not(target_os = "android"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tab {
    PullRequest,
    Targets,
    Log,
}

#[cfg(not(target_os = "android"))]
impl Tab {
    pub(crate) const ALL: [Tab; 3] = [Tab::PullRequest, Tab::Targets, Tab::Log];
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

type Slot<T> = (ReadSignal<T>, WriteSignal<T>);
pub(crate) type Cache<K, T> = Rc<RefCell<HashMap<K, Slot<Loaded<T>>>>>;

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
    #[cfg(not(target_os = "android"))]
    pub(crate) head: ReadSignal<String>,
    #[cfg(not(target_os = "android"))]
    set_head: WriteSignal<String>,
    #[cfg(not(target_os = "android"))]
    pub(crate) head_summary: ReadSignal<String>,
    #[cfg(not(target_os = "android"))]
    set_head_summary: WriteSignal<String>,
    #[cfg(not(target_os = "android"))]
    pub(crate) common: ReadSignal<usize>,
    #[cfg(not(target_os = "android"))]
    set_common: WriteSignal<usize>,
    pub(crate) viewer: ReadSignal<Option<Viewer>>,
    set_viewer: WriteSignal<Option<Viewer>>,
    #[cfg(not(target_os = "android"))]
    pub(crate) tab: ReadSignal<Tab>,
    #[cfg(not(target_os = "android"))]
    set_tab: WriteSignal<Tab>,
    #[cfg(not(target_os = "android"))]
    stopped: Rc<Cell<bool>>,
    #[cfg(not(target_os = "android"))]
    pub(crate) targets: ReadSignal<Option<Loaded<Vec<Target>>>>,
    #[cfg(not(target_os = "android"))]
    set_targets: WriteSignal<Option<Loaded<Vec<Target>>>>,
    #[cfg(not(target_os = "android"))]
    pub(crate) target_query: ReadSignal<String>,
    #[cfg(not(target_os = "android"))]
    pub(crate) set_target_query: WriteSignal<String>,
    #[cfg(not(target_os = "android"))]
    pub(crate) running: ReadSignal<bool>,
    #[cfg(not(target_os = "android"))]
    set_running: WriteSignal<bool>,
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
        let (repository, set_repository) = create_signal(Loaded::Loading);
        let (filter, set_filter) = create_signal(Filter::Open);
        let (listing, set_listing) = create_signal(Loaded::Loading);
        let (query, set_query) = create_signal(String::new());
        let (selected, set_selected) = create_signal(None::<PullRequest>);
        let selection = create_selector(clone!(selected -> move || {
            selected.with(|selected| selected.as_ref().map(|pull_request| pull_request.number))
        }));
        #[cfg(not(target_os = "android"))]
        let (head, set_head) = create_signal(String::new());
        #[cfg(not(target_os = "android"))]
        let (head_summary, set_head_summary) = create_signal(String::new());
        #[cfg(not(target_os = "android"))]
        let (common, set_common) = create_signal(0usize);
        #[cfg(not(target_os = "android"))]
        let (tab, set_tab) = create_signal(Tab::PullRequest);
        let (viewer, set_viewer) = create_signal(None);
        #[cfg(not(target_os = "android"))]
        let (targets, set_targets) = create_signal(None);
        #[cfg(not(target_os = "android"))]
        let (target_query, set_target_query) = create_signal(String::new());
        #[cfg(not(target_os = "android"))]
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
            #[cfg(not(target_os = "android"))]
            head,
            #[cfg(not(target_os = "android"))]
            set_head,
            #[cfg(not(target_os = "android"))]
            head_summary,
            #[cfg(not(target_os = "android"))]
            set_head_summary,
            #[cfg(not(target_os = "android"))]
            common,
            #[cfg(not(target_os = "android"))]
            set_common,
            viewer,
            set_viewer,
            #[cfg(not(target_os = "android"))]
            tab,
            #[cfg(not(target_os = "android"))]
            set_tab,
            #[cfg(not(target_os = "android"))]
            stopped: Rc::default(),
            #[cfg(not(target_os = "android"))]
            targets,
            #[cfg(not(target_os = "android"))]
            set_targets,
            #[cfg(not(target_os = "android"))]
            target_query,
            #[cfg(not(target_os = "android"))]
            set_target_query,
            #[cfg(not(target_os = "android"))]
            running,
            #[cfg(not(target_os = "android"))]
            set_running,
            #[cfg(not(target_os = "android"))]
            pane,
            #[cfg(target_os = "android")]
            phone,
        }
    }

    pub(crate) fn start(&self) {
        #[cfg(not(target_os = "android"))]
        self.pane
            .write_line("Output from ./scripts/switch and ./scripts/buck shows here.");
        self.tasks.connect();
        self.tasks.list(Filter::Open);
        #[cfg(not(target_os = "android"))]
        self.tasks.read_head();
        #[cfg(target_os = "android")]
        self.phone.start();
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
        #[cfg(not(target_os = "android"))]
        self.set_tab.set(Tab::PullRequest);
    }

    #[cfg(target_os = "android")]
    pub(crate) fn deselect(&self) {
        self.set_selected.set(None);
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
        #[cfg(target_os = "android")]
        self.phone.refresh(
            crate::builds::Slot::PullRequest(pull_request.number),
            &pull_request.head_sha,
        );
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

    #[cfg(not(target_os = "android"))]
    pub(crate) fn show_tab(&self, tab: Tab) {
        self.set_tab.set(tab);
        if tab == Tab::Targets && self.targets.get_untracked().is_none() {
            self.refresh_targets();
        }
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn refresh_targets(&self) {
        self.set_targets.set(Some(Loaded::Loading));
        self.tasks.targets();
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn run(&self, args: Vec<String>) {
        if args.is_empty() || self.running.get_untracked() {
            return;
        }
        self.set_running.set(true);
        self.tasks.run("buck", args);
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
            self.set_common.set(common.0);
            args.extend(words(common.1.args));
        }
        self.set_running.set(true);
        self.tasks.run("switch", args);
    }

    #[cfg(not(target_os = "android"))]
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

    #[cfg(not(target_os = "android"))]
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

    #[cfg(not(target_os = "android"))]
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
            #[cfg(not(target_os = "android"))]
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
            #[cfg(not(target_os = "android"))]
            Event::Started(description) => {
                self.set_running.set(true);
                self.stopped.set(false);
                self.pane
                    .write_line(&format!("{BOLD}$ {description}{RESET}"));
            }
            #[cfg(not(target_os = "android"))]
            Event::Targets(targets) => self.set_targets.set(Some(match targets {
                Ok(targets) => Loaded::Ready(targets),
                Err(error) => Loaded::Failed(error),
            })),
            #[cfg(not(target_os = "android"))]
            Event::Output(bytes) => self.pane.write(&bytes),
            #[cfg(not(target_os = "android"))]
            Event::Finished { summary, success } => {
                let color = if success { GREEN } else { RED };
                self.pane.write_line(&format!("{color}{summary}{RESET}\n"));
                self.set_running.set(false);
                if !success && !self.stopped.replace(false) {
                    self.set_tab.set(Tab::Log);
                }
            }
            #[cfg(target_os = "android")]
            Event::Phone(event) => self.phone.receive(event),
        }
    }
}
