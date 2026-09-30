use super::*;
use crate::reactive::{ReadSignal, create_memo};
use crate::unstyled::{Container, DockMode, DockState, TabId, dock_actions, narrower_than};

const PHONE: Vec2 = Vec2::new(390.0, 800.0);
const NARROW: f32 = 600.0;

#[component]
fn Docked() -> NodeId {
    let narrow = narrower_than(NARROW);
    let mode = create_memo(move || match narrow.get() {
        true => DockMode::Stacked,
        false => DockMode::Tiled,
    });
    let mut layout = DockState::new([TabId::new(1), TabId::new(2)]);
    layout.show(TabId::new(2));
    let (state, set_state) = create_signal(layout);
    view! {
        <styled::DockArea
            state={state}
            mode={mode}
            home={Some(TabId::new(1))}
            title={Func::new(|tab: TabId| format!("Tab {}", tab.value()))}
            on_change={move |next: DockState| set_state.set(next)}
            on_close={move |_: TabId| {}}
        >
            {move |tab: TabId| {
                let id = tab.value();
                dock_actions(view! {
                    <Frame @test_id={format!("action.{id}")} width=24.0 height=24.0 />
                });
                view! {
                    <Frame @test_id={format!("content.{id}")} />
                }
            }}
        </styled::DockArea>
    }
}

fn laid_out(harness: &Harness, test_id: &str) -> bool {
    harness
        .document()
        .find_test_id(test_id)
        .and_then(|node| harness.document().node_rect(node))
        .is_some()
}

#[test]
fn tiling_a_stacked_dock_with_its_switcher_open_closes_the_switcher() {
    let document = build(move || {
        view! {
            <Container>
                {move |_: ReadSignal<Vec2>| view! {
                    <Docked />
                }}
            </Container>
        }
    });
    let mut harness = Harness::sized(document, PHONE);
    harness.settle();
    let at = harness.center(harness.find("dock.switch"));
    harness.click(at);
    harness.settle();
    assert!(laid_out(&harness, "dock.switcher.tab.2"));

    *harness.viewport_mut() = WIDE_VIEWPORT;
    harness.settle();
    assert!(
        !laid_out(&harness, "dock.switcher.tab.2"),
        "the switcher goes with the stacked screen"
    );
    assert!(laid_out(&harness, "content.2"));

    *harness.viewport_mut() = PHONE;
    harness.settle();
    assert!(
        laid_out(&harness, "action.2"),
        "stacking the dock again brings the tab's actions back to its bar"
    );
}
