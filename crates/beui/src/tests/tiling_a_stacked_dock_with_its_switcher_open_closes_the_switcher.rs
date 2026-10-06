use super::*;
use crate::reactive::{ForEach, ReadSignal, create_memo};
use crate::unstyled::{
    Container, DockMode, DockPane, DockTab, DockingLayout, dock_actions, narrower_than,
};

const PHONE: Vec2 = Vec2::new(390.0, 800.0);
const NARROW: f32 = 600.0;

#[component]
fn Docked() -> NodeId {
    let narrow = narrower_than(NARROW);
    let mode = create_memo(move || match narrow.get() {
        true => DockMode::Stacked,
        false => DockMode::Tiled,
    });
    let layout = DockingLayout::new();
    view! {
        <styled::Docking layout mode={mode} home=1u64 focus=2u64>
            <DockPane id="tabs">
                <ForEach keys={vec![1u64, 2]}>
                    {move |id: u64| view! {
                        <DockTab
                            id
                            title={format!("Tab {id}")}
                            on_close={|| {}}
                            content={move || {
                                dock_actions(move || view! {
                                    <Frame
                                        @test_id={format!("action.{id}")}
                                        width=24.0
                                        height=24.0
                                    />
                                });
                                view! {
                                    <Frame @test_id={format!("content.{id}")} />
                                }
                            }}
                        />
                    }}
                </ForEach>
            </DockPane>
        </styled::Docking>
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
