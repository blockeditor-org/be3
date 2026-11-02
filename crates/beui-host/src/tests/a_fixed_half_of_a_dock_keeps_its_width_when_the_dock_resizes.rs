use super::*;
use crate::dock::unstyled::{DockPane, DockSplit, DockTab, DockingLayout, SplitSize};
use crate::reactive::*;

#[test]
fn a_fixed_half_of_a_dock_keeps_its_width_when_the_dock_resizes() {
    struct Prompt;
    impl Drop for Prompt {
        fn drop(&mut self) {
            crate::native::testing::delay_measurement(false);
        }
    }
    let _prompt = Prompt;
    crate::native::testing::delay_measurement(true);
    let _root = crate::reactive::RootScope::new();
    let left = NodeRef::new();
    let right = NodeRef::new();
    let document = build_node(move || {
        let layout = DockingLayout::new();
        view! {
            <dock::styled::Docking layout>
                <DockSplit id="split" size={SplitSize::First(300.0)}>
                    <DockPane id="left">
                        <DockTab id=1u64 title="Tab 1">
                            <Frame @node_ref=&left />
                        </DockTab>
                    </DockPane>
                    <DockPane id="right">
                        <DockTab id=2u64 title="Tab 2">
                            <Frame @node_ref=&right />
                        </DockTab>
                    </DockPane>
                </DockSplit>
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let (left, right) = (left.get(), right.get());
    let before = (harness.rect(left), harness.rect(right));
    let margin = WIDE_VIEWPORT.x - before.1.right();

    let wider = Vec2::new(1300.0, WIDE_VIEWPORT.y);
    harness.resize(wider);
    harness.frame(Vec::new());
    let shown = (harness.rect(left), harness.rect(right));
    assert_eq!(
        shown.0.width(),
        before.0.width(),
        "the fixed half keeps its width in the frame of the resize: {shown:?} from {before:?}"
    );
    assert_eq!(
        shown.1.right(),
        wider.x - margin,
        "the other half takes the room the dock gained: {shown:?}"
    );

    let narrower = Vec2::new(800.0, WIDE_VIEWPORT.y);
    harness.resize(narrower);
    harness.frame(Vec::new());
    let shown = (harness.rect(left), harness.rect(right));
    assert_eq!(
        shown.0.width(),
        before.0.width(),
        "the fixed half keeps its width when the dock narrows: {shown:?}"
    );
    assert_eq!(
        shown.1.right(),
        narrower.x - margin,
        "and the other half gives up the room: {shown:?}"
    );
}
