use std::cell::RefCell;
use std::rc::Rc;

use uuid::Uuid;

use crate::state::{Callbacks, Subscription};
use crate::{Artifact, BlockGraph, EditorHost};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Changed {
    Settings,
    Saved,
}

#[derive(Clone)]
pub struct ArtifactsCore(Rc<Inner>);

struct Inner {
    host: EditorHost,
    artifact: Artifact,
    settings: RefCell<Vec<u8>>,
    saved: RefCell<Vec<u8>>,
    edit: RefCell<Option<Vec<u8>>>,
    #[allow(clippy::type_complexity)] // spelled out where it is used; a name would add nothing
    changed: Callbacks<Changed, dyn Fn(&[u8])>,
}

impl ArtifactsCore {
    pub fn new(host: EditorHost, artifact: Artifact) -> Self {
        Self(Rc::new(Inner {
            host,
            artifact,
            settings: RefCell::default(),
            saved: RefCell::default(),
            edit: RefCell::new(None),
            changed: Callbacks::default(),
        }))
    }

    pub fn settings(&self) -> Vec<u8> {
        self.0.settings.borrow().clone()
    }

    pub fn saved_settings(&self) -> Vec<u8> {
        self.0.saved.borrow().clone()
    }

    pub fn on_settings(&self, run: impl Fn(&[u8]) + 'static) -> Subscription {
        self.0.changed.add(Changed::Settings, Rc::new(run))
    }

    pub fn on_saved_settings(&self, run: impl Fn(&[u8]) + 'static) -> Subscription {
        self.0.changed.add(Changed::Saved, Rc::new(run))
    }

    fn change(&self, which: Changed, data: &[u8]) {
        let held = match which {
            Changed::Settings => &self.0.settings,
            Changed::Saved => &self.0.saved,
        };
        if held.borrow().as_slice() == data {
            return;
        }
        *held.borrow_mut() = data.to_vec();
        for run in self.0.changed.of([&which]) {
            run(data);
        }
    }

    pub fn edit_settings(&self, data: Vec<u8>) {
        self.change(Changed::Settings, &data);
        *self.0.edit.borrow_mut() = Some(data);
    }

    pub fn receive_settings(&self, saved: &[u8], draft: &[u8]) {
        self.change(Changed::Saved, saved);
        self.change(Changed::Settings, draft);
    }

    pub fn take_settings_edit(&self) -> Option<Vec<u8>> {
        self.0.edit.borrow_mut().take()
    }

    pub fn host(&self) -> &EditorHost {
        &self.0.host
    }

    pub fn graph(&self) -> BlockGraph {
        self.0.host.graph()
    }

    pub fn block_id(&self) -> Uuid {
        self.0.artifact.block_id
    }

    pub fn block_type(&self) -> Uuid {
        self.0.artifact.block_type
    }

    pub fn source(&self) -> Uuid {
        self.0.artifact.source
    }

    pub fn artifact(&self) -> &Artifact {
        &self.0.artifact
    }

    /// The exporter of the source's editor that makes the artifact.
    pub fn exporter(&self) -> &str {
        &self.0.artifact.exporter
    }
}
