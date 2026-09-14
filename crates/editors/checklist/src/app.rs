use std::rc::Rc;
use std::sync::Arc;

use block_client::blocks::checklist::{Checklist, ChecklistOperation};
use block_editor_plugin::EditorHost;
use block_editor_plugin::beui::{Context, Rect};
use block_reactive::BlockSource;
use uuid::Uuid;

mod ui;

use ui::ChecklistUi;

#[derive(Default)]
pub struct ChecklistApp {
    ui: Option<ChecklistUi>,
    checklist: Option<Rc<ChecklistEditor>>,
    creation: Option<Arc<block_client::BlockClient>>,
}

impl ChecklistApp {
    pub fn ui(&self) -> Option<&ChecklistUi> {
        self.ui.as_ref()
    }
}

pub(crate) struct ChecklistEditor {
    source: Rc<BlockSource<Checklist>>,
    host: EditorHost,
}

impl ChecklistEditor {
    pub(crate) fn source(&self) -> &Rc<BlockSource<Checklist>> {
        &self.source
    }

    fn operate(&self, operation: ChecklistOperation) {
        if self.host.editable() {
            self.source.operate(operation);
        }
    }

    pub(crate) fn add(&self, text: String) {
        self.operate(ChecklistOperation::add(text));
    }

    pub(crate) fn set_done(&self, id: Uuid, done: bool) {
        self.operate(ChecklistOperation::SetDone { id, done });
    }

    pub(crate) fn remove(&self, id: Uuid) {
        self.operate(ChecklistOperation::Remove { id });
    }

    pub(crate) fn clear_done(&self) {
        self.operate(ChecklistOperation::ClearDone);
    }
}

impl block_editor_plugin::BeuiApp for ChecklistApp {
    fn connect(
        &mut self,
        host: EditorHost,
        client: Arc<block_client::BlockClient>,
        block_id: Uuid,
    ) {
        let waker = host.waker();
        let source = BlockSource::new(client.get_block(block_id), move || waker.wake());
        self.checklist = Some(Rc::new(ChecklistEditor { source, host }));
        self.ui = None;
    }

    fn connect_creation(&mut self, _host: EditorHost, client: Arc<block_client::BlockClient>) {
        self.creation = Some(client);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let client = self
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        Ok(client.create_block(Checklist::default()).id())
    }

    fn frame(&mut self, context: &Context, rect: Rect) {
        let Some(checklist) = self.checklist.clone() else {
            return;
        };
        let ui = self
            .ui
            .get_or_insert_with(|| ChecklistUi::new(checklist.clone()));
        ui.pump();
        ui.show(context, rect);
    }
}
