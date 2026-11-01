use block_ui_test::BeuiTest;
use logic_grid_types::hotbar::{Hotbar, HotbarContent, HotbarSlot};
use uuid::Uuid;

use crate::hotbar::app::HotbarApp;

mod unpinning_a_component_from_the_hotbar_takes_its_row_away;

fn editor(slots: Vec<HotbarSlot>) -> BeuiTest<HotbarApp> {
    let block = Uuid::new_v4();
    let mut harness = BeuiTest::editing(block);
    harness.hold(
        None,
        HotbarContent::new(&Hotbar {
            slots: slots.into_iter().collect(),
        }),
    );
    harness.run();
    harness
}
