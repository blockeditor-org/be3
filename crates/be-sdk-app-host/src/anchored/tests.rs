use be_lib_geometry::pos2;

use super::*;

mod a_press_in_a_menu_closes_its_submenu_and_one_outside_closes_both;

/// A view that takes presses over `rect`, closed by a press outside it.
fn hung(id: u64, parent: Option<u64>, rect: Rect) -> Hung {
    Hung {
        id: ViewId(id),
        parent: parent.map(ViewId),
        floating: vec![rect],
        dismiss_on_outside_press: true,
        trigger: None,
        blocks_owner: false,
    }
}
