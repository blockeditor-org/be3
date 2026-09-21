use block_editor_plugin::beui::unstyled::{DockState, TabId};

use crate::app::workspace::{FILES, place_tab, set_files_compact, settled, starting_layout};

const BLOCK: TabId = TabId::new(9);

#[test]
fn a_narrow_workspace_folds_files_into_the_pane_beside_it() {
    let mut layout = starting_layout();
    place_tab(&mut layout, BLOCK);
    layout = settled(layout);
    assert_ne!(
        pane_of(&layout, FILES),
        pane_of(&layout, BLOCK),
        "a wide workspace gives files a pane of its own"
    );

    set_files_compact(&mut layout, true);

    assert_eq!(
        pane_of(&layout, FILES),
        pane_of(&layout, BLOCK),
        "a narrow workspace folds files into the tabs beside it"
    );

    set_files_compact(&mut layout, false);

    assert_ne!(
        pane_of(&layout, FILES),
        pane_of(&layout, BLOCK),
        "widening again splits files back out into its own pane"
    );
}

fn pane_of(layout: &DockState, tab: TabId) -> usize {
    let position = layout
        .find(tab)
        .unwrap_or_else(|| panic!("{tab:?} is in the layout"));
    layout
        .surfaces()
        .into_iter()
        .flat_map(|surface| layout.leaves(surface))
        .position(|leaf| leaf == position.leaf)
        .expect("the pane holding a tab is one of the layout's panes")
}
