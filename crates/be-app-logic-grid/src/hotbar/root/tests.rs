use be_sdk_test::BeuiTest;
use be_type_logic_grid::hotbar::{Hotbar, HotbarContent, HotbarSlot};
use uuid::Uuid;

use crate::hotbar::root::HotbarRoot;

mod unpinning_a_component_from_the_hotbar_takes_its_row_away;

fn editor(slots: Vec<HotbarSlot>) -> BeuiTest<HotbarRoot> {
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
