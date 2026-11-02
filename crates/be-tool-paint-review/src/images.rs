use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use beui::Image;
use beui::reactive::*;

#[derive(Clone, Debug, PartialEq)]
pub enum ImageState {
    Loading,
    Ready(Image),
    Failed(String),
}

pub struct ImageRequest {
    hash: String,
    images: Images,
}

impl ImageRequest {
    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn deliver(&self, image: Result<Image, String>) {
        self.images.deliver(&self.hash, image);
    }
}

pub(crate) struct ImageTable {
    states: RefCell<HashMap<String, OwnedSignal<ImageState>>>,
    requested: RefCell<HashSet<String>>,
    request: Box<dyn Fn(ImageRequest)>,
}

#[derive(Clone, Copy)]
pub struct Images(Stored<ImageTable>);

impl Images {
    pub(crate) fn owned(request: impl Fn(ImageRequest) + 'static) -> OwnedStored<ImageTable> {
        OwnedStored::new(ImageTable {
            states: RefCell::new(HashMap::new()),
            requested: RefCell::new(HashSet::new()),
            request: Box::new(request),
        })
    }

    pub(crate) fn new(table: Stored<ImageTable>) -> Self {
        Self(table)
    }

    pub fn deliver(self, hash: &str, image: Result<Image, String>) {
        let state = match image {
            Ok(image) => ImageState::Ready(image),
            Err(error) => ImageState::Failed(error),
        };
        self.signal(hash).set(state);
    }

    pub(crate) fn state(self, hash: &str) -> Signal<ImageState> {
        self.signal(hash)
    }

    pub(crate) fn want(self, hash: &str) {
        let asked = self
            .0
            .with(|table| table.requested.borrow_mut().insert(hash.to_owned()));
        if asked {
            let request = ImageRequest {
                hash: hash.to_owned(),
                images: self,
            };
            self.0.with(|table| (table.request)(request));
        }
    }

    fn signal(self, hash: &str) -> Signal<ImageState> {
        self.0.with(|table| {
            table
                .states
                .borrow_mut()
                .entry(hash.to_owned())
                .or_insert_with(|| OwnedSignal::new(ImageState::Loading))
                .handle()
        })
    }
}
