use super::*;
use crate::dock::unstyled::{DockPane, DockSplit, DockTab, DockingLayout};
use crate::reactive::*;

#[test]
fn a_resized_dock_lays_its_panes_out_for_the_new_size_in_the_same_frame() {
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
                <DockSplit id="split">
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
        shown.1.right(),
        wider.x - margin,
        "the right pane reaches the new edge in the frame of the resize: {shown:?} from {before:?}"
    );
    assert!(
        (shown.0.width() - shown.1.width()).abs() <= 1.0,
        "the split still halves the wider dock: {shown:?}"
    );

    harness.frame(Vec::new());
    assert_eq!(
        (harness.rect(left), harness.rect(right)),
        shown,
        "the next frame lays the panes out where the first one did"
    );
}
