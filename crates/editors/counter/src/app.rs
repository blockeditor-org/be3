use std::rc::Rc;
use std::sync::Arc;

use block_client::blocks::counter::{Counter as CounterBlock, CounterOperation};
use block_editor_plugin::EditorHost;
use block_editor_plugin::beui::{Context, Rect};
use block_reactive::BlockSource;
use uuid::Uuid;

mod ui;

use ui::CounterUi;

#[derive(Default)]
pub struct CounterApp {
    ui: Option<CounterUi>,
    counter: Option<Rc<CounterEditor>>,
    creation: Option<Arc<block_client::BlockClient>>,
}

impl CounterApp {
    pub fn ui(&self) -> Option<&CounterUi> {
        self.ui.as_ref()
    }
}

pub(crate) struct CounterEditor {
    source: Rc<BlockSource<CounterBlock>>,
    host: EditorHost,
}

impl CounterEditor {
    pub(crate) fn source(&self) -> &Rc<BlockSource<CounterBlock>> {
        &self.source
    }

    fn operate(&self, operation: CounterOperation) {
        if self.host.editable() {
            self.source.operate(operation);
        }
    }

    pub(crate) fn increment(&self) {
        self.operate(CounterOperation::Increment);
    }

    pub(crate) fn decrement(&self) {
        self.operate(CounterOperation::Decrement);
    }

    pub(crate) fn reset(&self) {
        self.operate(CounterOperation::Reset);
    }
}

impl block_editor_plugin::BeuiApp for CounterApp {
    fn connect(
        &mut self,
        host: EditorHost,
        client: Arc<block_client::BlockClient>,
        block_id: Uuid,
    ) {
        let waker = host.waker();
        let source = BlockSource::new(client.get_block(block_id), move || waker.wake());
        self.counter = Some(Rc::new(CounterEditor { source, host }));
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
        Ok(client.create_block(CounterBlock::default()).id())
    }

    fn frame(&mut self, context: &Context, rect: Rect) {
        let Some(counter) = self.counter.clone() else {
            return;
        };
        let ui = self
            .ui
            .get_or_insert_with(|| CounterUi::new(counter.clone()));
        ui.pump();
        ui.show(context, rect);
    }
}
