use block_editor_beui::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::FileTreeApp;

mod clicking_the_chevron_opens_and_closes_its_own_row;
mod expanding_a_folder_shows_its_children_without_more_input;
mod inspecting_a_row_shows_what_is_known_about_its_block;

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
    let tree = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), tree);
    let mut fixture = Fixture {
        test: BeuiTest::new(editor),
        host,
    };
    fixture.settle();
    fixture
}
