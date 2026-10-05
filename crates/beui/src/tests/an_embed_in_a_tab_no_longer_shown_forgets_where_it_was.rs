use super::*;
use crate::reactive::{Embed, EmbedSlot, view, with_reactive_scope};
use crate::unstyled::{DockGroup, DockPane, DockTab, DockingLayout};

#[test]
fn an_embed_in_a_tab_no_longer_shown_forgets_where_it_was() {
    let first = EmbedSlot::new();
    let second = EmbedSlot::new();
    let slots = (first.clone(), second.clone());
    let layout = DockingLayout::new();
    let showing = layout.clone();
    let returning = layout.clone();
    let inside = layout.clone();
    let document = build(move || {
        let (first, second) = slots.clone();
        view! {
            <styled::Docking layout focus=1u64>
                <DockPane id="main">
                    <DockTab id=1u64 title="Tab 1">
                        <Embed slot={first.clone()} />
                    </DockTab>
                    <DockGroup id="group">
                        <DockPane id="grouped" active=3u64>
                            <DockTab id=2u64 title="Tab 2">
                                <Embed slot={second.clone()} />
                            </DockTab>
                            <DockTab id=3u64 title="Tab 3">
                                <Embed slot={EmbedSlot::new()} />
                            </DockTab>
                        </DockPane>
                    </DockGroup>
                </DockPane>
            </styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    assert!(first.placement().is_some(), "the tab on show is placed");

    with_reactive_scope(harness.document_mut(), move || {
        showing.show(&2);
    });
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert!(second.placement().is_some());
    assert!(
        first.placement().is_none(),
        "a tab hidden behind a group no longer claims the place it was shown at"
    );

    with_reactive_scope(harness.document_mut(), move || {
        returning.show(&1);
    });
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert!(
        first.placement().is_some(),
        "the tab shown again is placed again"
    );
    assert!(second.placement().is_none());

    with_reactive_scope(harness.document_mut(), move || {
        inside.show(&2);
    });
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert!(
        second.placement().is_some(),
        "the tab inside the group shown again is placed again"
    );
}
