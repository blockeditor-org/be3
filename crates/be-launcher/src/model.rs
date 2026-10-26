use std::cell::RefCell;
use std::collections::HashMap;

use beui::Image;
use beui::reactive::*;
#[cfg(not(target_os = "android"))]
use beui::dock::unstyled::{DockDocuments, DockSlot, DockingLayout};

use crate::builds::Slot;
use crate::github::{Entry, Filter, PullRequest, timeline_images};
use crate::runs::Runs;
use crate::tasks::{Event, Tasks, open_url};

#[cfg(not(target_os = "android"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Tab {
    PullRequest(DockSlot),
    Log,
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
    #[cfg(target_os = "android")]
    pub(crate) opened: Signal<Vec<PullRequest>>,
    pub(crate) selection: Selector<Option<u64>>,
    timelines: Cache<u64, Vec<Entry>>,
    images: Cache<String, Image>,
    pub(crate) viewer: Signal<Option<Viewer>>,
    #[cfg(not(target_os = "android"))]
    pub(crate) layout: DockingLayout<Tab>,
    #[cfg(not(target_os = "android"))]
    pub(crate) documents: DockDocuments<Tab, PullRequest, u64>,
    pub(crate) runs: Runs,
}

impl Model {
    pub(crate) fn new(tasks: Tasks, runs: Runs) -> Self {
        let repository = create_signal(Loaded::Loading);
        let filter = create_signal(Filter::Open);
        let listing = create_signal(Loaded::Loading);
        let query = create_signal(String::new());
        #[cfg(target_os = "android")]
        let opened = create_signal(Vec::<PullRequest>::new());
        #[cfg(not(target_os = "android"))]
        let layout = DockingLayout::new();
        #[cfg(not(target_os = "android"))]
        let documents =
            DockDocuments::new(layout, Tab::PullRequest, |pull_request: &PullRequest| {
                pull_request.number
            });
        #[cfg(not(target_os = "android"))]
        let selection = create_selector(move || {
            shown_pull_request(documents).map(|pull_request| pull_request.number)
        });
        #[cfg(target_os = "android")]
        let selection = create_selector(move || {
            opened.with(|opened| opened.first().map(|pull_request| pull_request.number))
        });
        Self {
            tasks: store(tasks),
            owner: store(owner_scope()),
            repository,
            filter,
            listing,
            pull_requests: KeyedStore::new(),
            query,
            #[cfg(target_os = "android")]
            opened,
            selection,
            timelines: store(RefCell::default()),
            images: store(RefCell::default()),
            viewer: create_signal(None),
            #[cfg(not(target_os = "android"))]
            layout,
            #[cfg(not(target_os = "android"))]
            documents,
            runs,
        }
    }

    pub(crate) fn start(&self) {
        self.runs
            .pane
            .write_line("The log of each build on the build helper shows here.");
        self.tasks.value().connect();
        self.tasks.value().list(Filter::Open);
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
    }

    #[cfg(target_os = "android")]
    pub(crate) fn select(&self, number: u64, _beside: bool) {
        if let Some(pull_request) = self.pull_requests.try_get(&number) {
            self.opened.set(vec![pull_request.get_untracked()]);
        }
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn select(&self, number: u64, beside: bool) {
        if let Some(pull_request) = self.pull_requests.try_get(&number) {
            self.documents.open(pull_request.get_untracked(), beside);
        }
    }

    #[cfg(target_os = "android")]
    pub(crate) fn deselect(&self) {
        self.opened.set(Vec::new());
    }

    pub(crate) fn current(&self, pull_request: &PullRequest) -> bool {
        self.runs.holds(Slot::PullRequest(pull_request.number))
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

    pub(crate) fn open(&self, url: &str) {
        open_url(url);
    }

    #[cfg(not(target_os = "android"))]
    fn refresh_opened(&self, listed: &[PullRequest]) {
        let documents = self.documents;
        for slot in documents.slots().get_untracked() {
            let Some(held) = documents.content(slot) else {
                continue;
            };
            let number = held.with_untracked(|held| held.number);
            if let Some(refreshed) = listed
                .iter()
                .find(|pull_request| pull_request.number == number)
            {
                documents.refresh(refreshed.clone());
            }
        }
    }

    #[cfg(target_os = "android")]
    fn refresh_opened(&self, listed: &[PullRequest]) {
        let stale = self.opened.with_untracked(|opened| {
            opened.iter().any(|held| {
                listed
                    .iter()
                    .any(|pull_request| pull_request.number == held.number && pull_request != held)
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
                        self.refresh_opened(&listed);
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
            Event::Run(event) => {
                let failed = matches!(event, crate::runs::Event::Ready(Err(_)));
                self.runs.receive(event);
                #[cfg(not(target_os = "android"))]
                if failed {
                    self.layout.show(&Tab::Log);
                }
                #[cfg(target_os = "android")]
                let _ = failed;
            }
        }
    }
}

#[cfg(not(target_os = "android"))]
fn shown_pull_request(documents: DockDocuments<Tab, PullRequest, u64>) -> Option<PullRequest> {
    let slot = documents.shown()?;
    Some(documents.content(slot)?.get())
}
