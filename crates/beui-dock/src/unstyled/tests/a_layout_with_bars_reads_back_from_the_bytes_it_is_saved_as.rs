use super::*;
use crate::unstyled::state::DockSpecBar;

fn bar(bar: u64, owner: Option<u64>, placement: DockBarPlacement) -> DockSpecBar {
    DockSpecBar {
        bar: TabId::new(bar),
        owner: owner.map(TabId::new),
        placement,
        hidden: false,
    }
}

#[test]
fn a_layout_with_bars_reads_back_from_the_bytes_it_is_saved_as() {
    let mut state = DockState::default();
    state.reconcile(&DockSpec {
        bars: vec![
            bar(
                20,
                None,
                DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::Start),
            ),
            bar(
                21,
                None,
                DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::End),
            ),
            bar(
                22,
                Some(1),
                DockBarPlacement::Over(DockEdge::Top, DockAlign::Center),
            ),
        ],
        ..spec(pane("main", &[1, 2]))
    });
    state.set_bar_hidden(TabId::new(21), true);
    state.set_bar_placement(
        TabId::new(20),
        DockBarPlacement::Beside(DockEdge::Left, DockAlign::Center),
    );
    state.set_bar_placement(
        TabId::new(22),
        DockBarPlacement::Window {
            edge: DockEdge::Right,
            origin: beui_view::types::pos2(40.0, 60.0),
        },
    );

    let bytes = postcard::to_stdvec(&state).expect("the layout is written");
    let read: DockState = postcard::from_bytes(&bytes).expect("the layout is read back");
    assert_eq!(
        read, state,
        "postcard, the format a view's saved state uses, reads back a layout's bars: hidden, placed, the dock's own and a tab's"
    );
}
