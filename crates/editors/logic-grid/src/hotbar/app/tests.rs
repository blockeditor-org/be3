use block_editor_beui::be_block::hotbar::{Hotbar, HotbarContent, HotbarSlot};
use block_editor_beui::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::hotbar::app::HotbarApp;

mod unpinning_a_component_from_the_hotbar_takes_its_row_away;

fn editor(slots: Vec<HotbarSlot>) -> BeuiTest<HotbarApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), block);
    let mut harness = BeuiTest::new(editor);
    harness.hold(
        None,
        HotbarContent::new(&Hotbar {
            slots: slots.into_iter().collect(),
        }),
    );
    harness.run();
    harness
}
