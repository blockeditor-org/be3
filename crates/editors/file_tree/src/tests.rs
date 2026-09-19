use std::sync::Arc;

use block_client::BlockClient;
use block_client::blocks::counter::Counter;
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::FileTreeApp;

mod clicking_the_chevron_opens_and_closes_its_own_row;

struct Fixture {
    test: BeuiTest<FileTreeApp>,
    host: EditorHost,
}

impl Fixture {
    fn settle(&mut self) {
        for _ in 0..6 {
            self.test.run();
        }
    }

    fn opened(&self) -> Vec<Uuid> {
        self.host
            .take_opens()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect()
    }
}

fn editor() -> Fixture {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let tree = client.create_block(Counter::default());
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), client, tree.id());
    let mut fixture = Fixture {
        test: BeuiTest::new(editor),
        host,
    };
    fixture.settle();
    fixture
}
