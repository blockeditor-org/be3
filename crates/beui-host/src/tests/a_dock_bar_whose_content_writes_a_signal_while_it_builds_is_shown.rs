use super::*;
use crate::dock::unstyled::{
    DockAlign, DockBar, DockBarPlacement, DockEdge, DockPane, DockSplit, DockTab, DockingLayout,
    SplitSize,
};

#[component]
fn CountingBar() -> NodeId {
    let built = create_signal(0u32);
    built.set(1);
    let label = create_memo(move || format!("built {}", built.get()));
    view! {
        <Frame @test_id="bar.counting">
            <Text>{label}</Text>
        </Frame>
    }
}

#[test]
fn a_dock_bar_whose_content_writes_a_signal_while_it_builds_is_shown() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        let layout = DockingLayout::new();
        view! {
            <dock::styled::Docking layout focus=1u64>
                <DockSplit id="root" size={SplitSize::Fraction(0.6)}>
                    <DockPane id="stage" locked=true bare=true>
                        <DockTab id=1u64 title="Stage">
                            <Frame @test_id="content.1" />
                        </DockTab>
                    </DockPane>
                    <DockPane id="side" locked=true>
                        <DockTab id=2u64 title="Side">
                            <Frame @test_id="content.2" />
                        </DockTab>
                    </DockPane>
                </DockSplit>
                <DockBar
                    id=9u64
                    parent=1u64
                    placement={DockBarPlacement::Beside(DockEdge::Top, DockAlign::Start)}
                >
                    <CountingBar />
                </DockBar>
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        harness.document().find_test_id("bar.counting").is_some(),
        "the bar's content is built outside any memo, so it may write signals"
    );
}
