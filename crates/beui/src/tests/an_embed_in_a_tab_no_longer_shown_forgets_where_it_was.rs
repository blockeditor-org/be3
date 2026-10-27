use super::*;
use crate::dock::unstyled::{DockGroup, DockPane, DockTab, DockingLayout};
use crate::reactive::*;

#[test]
fn an_embed_in_a_tab_no_longer_shown_forgets_where_it_was() {
    let _root = crate::reactive::RootScope::new();
    let first = EmbedSlot::new();
    let second = EmbedSlot::new();
    let slots = (first, second);
    let layout = DockingLayout::new();
    let document = build_node(move || {
        let (first, second) = slots;
        view! {
            <dock::styled::Docking layout focus=1u64>
                <DockPane id="main">
                    <DockTab id=1u64 title="Tab 1">
                        <Embed slot={first} />
                    </DockTab>
                    <DockGroup id="group">
                        <DockPane id="grouped" active=3u64>
                            <DockTab id=2u64 title="Tab 2">
                                <Embed slot={second} />
                            </DockTab>
                            <DockTab id=3u64 title="Tab 3">
                                <Embed slot={EmbedSlot::new()} />
                            </DockTab>
                        </DockPane>
                    </DockGroup>
                </DockPane>
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    assert!(first.placement().is_some(), "the tab on show is placed");

    layout.show(&2);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert!(second.placement().is_some());
    assert!(
        first.placement().is_none(),
        "a tab hidden behind a group no longer claims the place it was shown at"
    );

    layout.show(&1);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert!(
        first.placement().is_some(),
        "the tab shown again is placed again"
    );
    assert!(second.placement().is_none());

    layout.show(&2);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert!(
        second.placement().is_some(),
        "the tab inside the group shown again is placed again"
    );
}
