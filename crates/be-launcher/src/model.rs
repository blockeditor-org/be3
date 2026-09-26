use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use beui::Image;
use beui::reactive::{
    KeyedStore, ReadSignal, Selector, WriteSignal, clone, create_selector, create_signal,
};

use crate::github::{Entry, Filter, PullRequest};
use crate::pane::Pane;
use crate::tasks::{Event, Tasks, open_url};

const BOLD: &str = "\x1b[1m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Preset {
    pub(crate) title: &'static str,
    pub(crate) args: &'static str,
}

pub(crate) const PRESETS: [Preset; 10] = [
    Preset {
        title: "Run the app",
        args: "run //crates/block-app:app",
    },
    Preset {
        title: "Check that everything compiles",
        args: "run //:check",
    },
    Preset {
        title: "Verify: autofix, lint and test",
        args: "run //:verify",
    },
    Preset {
        title: "Smoke test the app",
        args: "run //crates/block-app:smoke",
    },
    Preset {
        title: "Serve the web app",
        args: "run //crates/block-app:web-serve",
    },
    Preset {
        title: "Install the app on Android",
        args: "run //crates/block-app:android -- --install",
    },
    Preset {
        title: "beui component demo",
        args: "run //crates/beui:demo-example",
    },
    Preset {
        title: "beui dock demo",
        args: "run //crates/beui:dock-example",
    },
    Preset {
        title: "beui survey example",
        args: "run //crates/beui:survey-example",
    },
    Preset {
        title: "Custom command",
        args: "",
    },
];

pub(crate) const CUSTOM: usize = PRESETS.len() - 1;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Loaded<T> {
    Loading,
    Ready(T),
    Failed(String),
}

type Slot<T> = (ReadSignal<T>, WriteSignal<T>);
type Cache<K, T> = Rc<RefCell<HashMap<K, Slot<Loaded<T>>>>>;

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
    pub(crate) preset: ReadSignal<usize>,
    pub(crate) set_preset: WriteSignal<usize>,
    pub(crate) custom: ReadSignal<String>,
    pub(crate) set_custom: WriteSignal<String>,
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
        let (preset, set_preset) = create_signal(0usize);
        let (custom, set_custom) = create_signal(String::new());
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
            preset,
            set_preset,
            custom,
            set_custom,
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

    pub(crate) fn command(&self) -> Vec<String> {
        let preset = self.preset.get_untracked();
        let args = match PRESETS.get(preset) {
            Some(preset) if preset.args.is_empty() => self.custom.get_untracked(),
            Some(preset) => preset.args.to_owned(),
            None => String::new(),
        };
        args.split_whitespace().map(str::to_owned).collect()
    }

    pub(crate) fn run(&self) {
        let args = self.command();
        if args.is_empty() || self.running.get_untracked() {
            return;
        }
        self.set_running.set(true);
        self.tasks.run("buck", args);
    }

    pub(crate) fn check_out(&self, pull_request: &PullRequest, then_run: bool) {
        if self.running.get_untracked() {
            return;
        }
        let mut args = vec![pull_request.branch.clone()];
        if then_run {
            let command = self.command();
            if command.is_empty() {
                return;
            }
            args.extend(command);
        }
        self.set_running.set(true);
        self.tasks.run("switch", args);
    }

    pub(crate) fn stop(&self) {
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
                self.set_head.set(sha);
                self.set_head_summary.set(summary);
            }
            Event::Started(description) => {
                self.set_running.set(true);
                self.pane
                    .write_line(&format!("{BOLD}$ {description}{RESET}"));
            }
            Event::Output(bytes) => self.pane.write(&bytes),
            Event::Finished { summary, success } => {
                let color = if success { GREEN } else { RED };
                self.pane.write_line(&format!("{color}{summary}{RESET}\n"));
                self.set_running.set(false);
            }
        }
    }
}
