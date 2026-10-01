use super::*;
use crate::reactive::ClickCallback;
use crate::unstyled::{DockMode, DockState, TabId, dock_more};

const HOME: TabId = TabId::new(1);

fn laid_out(harness: &Harness, test_id: &str) -> bool {
    harness
        .document()
        .find_test_id(test_id)
        .and_then(|node| harness.document().node_rect(node))
        .is_some()
}

#[test]
fn a_stacked_dock_bar_shows_more_for_a_tab_that_asks_and_presses_it() {
    let pressed = Rc::new(Cell::new(0));
    let pressing = pressed.clone();
    let document = build(move || {
        let mut layout = DockState::new([HOME, TabId::new(2)]);
        layout.show(TabId::new(2));
        let (state, set_state) = create_signal(layout);
        view! {
            <styled::DockArea
                state={state}
                mode=DockMode::Stacked
                home={Some(HOME)}
                title={Func::new(|tab: TabId| format!("Tab {}", tab.value()))}
                on_change={move |next: DockState| set_state.set(next)}
                on_close={move |_: TabId| {}}
            >
                {move |tab: TabId| {
                    if tab != HOME {
                        let pressing = pressing.clone();
                        dock_more(ClickCallback::new(move || pressing.set(pressing.get() + 1)));
                    }
                    view! {
                        <Frame @test_id={format!("content.{}", tab.value())} />
                    }
                }}
            </styled::DockArea>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();

    assert!(
        laid_out(&harness, "dock.more"),
        "the tab asked for a More button"
    );
    let at = harness.center(harness.find("dock.more"));
    harness.click(at);
    harness.settle();
    assert_eq!(pressed.get(), 1, "pressing More reaches the tab");

    let back = harness.center(harness.find("dock.back"));
    harness.click(back);
    harness.settle();

    assert!(laid_out(&harness, "content.1"));
    assert!(
        !laid_out(&harness, "dock.more"),
        "the home tab asked for no More button"
    );
}
