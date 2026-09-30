use super::*;
use crate::reactive::{Embed, EmbedSlot, Func, create_signal, view, with_reactive_scope};
use crate::unstyled::{DockDrop, DockState, TabId};

#[test]
fn an_embed_in_a_tab_no_longer_shown_forgets_where_it_was() {
    let first = EmbedSlot::new();
    let second = EmbedSlot::new();
    let slots = (first.clone(), second.clone());
    let mut layout = DockState::new([TabId::new(1), TabId::new(2), TabId::new(3)]);
    let leaf = layout.leaves(layout.main())[0];
    layout.drop_tab(TabId::new(3), DockDrop::Group { leaf, index: 1 });
    layout.show(TabId::new(1));
    let (state, set_state) = create_signal(layout);
    let showing = set_state.clone();
    let document = build(move || {
        let slots = slots.clone();
        view! {
            <styled::DockArea
                state={state}
                title={Func::new(|tab: TabId| format!("Tab {}", tab.value()))}
                on_change={move |next: DockState| set_state.set(next)}
                on_close={move |_: TabId| {}}
            >
                {move |tab: TabId| {
                    let slot = match tab.value() {
                        1 => slots.0.clone(),
                        2 => slots.1.clone(),
                        _ => EmbedSlot::new(),
                    };
                    view! {
                        <Embed slot={slot} />
                    }
                }}
            </styled::DockArea>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    assert!(first.placement().is_some(), "the tab on show is placed");

    with_reactive_scope(harness.document_mut(), move || {
        showing.update(|state| state.show(TabId::new(2)));
    });
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert!(second.placement().is_some());
    assert!(
        first.placement().is_none(),
        "a tab hidden behind a group no longer claims the place it was shown at"
    );
}
