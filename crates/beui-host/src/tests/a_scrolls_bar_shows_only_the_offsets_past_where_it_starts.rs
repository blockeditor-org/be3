use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{Scroll, ScrollHandle, ScrollStyle};

/// A 200-point scroll over 1000 points whose bar starts at 300: the bar reads the 700 points
/// past it as its scroll, and stays hidden while the scroll is short of it.
#[test]
fn a_scrolls_bar_shows_only_the_offsets_past_where_it_starts() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let shown = StoredCell::new(None::<(Memo<ScrollPosition>, ReadSignal<f32>, Callback<f32>)>);
    let document = build_node(move || {
        let style = ScrollStyle::new(crate::styled::Movement::Rubber.spring()).bar(
            8.0,
            RenderFn::new(move |handle: ScrollHandle| {
                shown.set(Some((handle.position, handle.visibility, handle.scroll_to)));
                view! { <Frame /> }
            }),
        );
        view! {
            <List spacing=0.0>
                <ListItem length=200.0>
                    <Scroll @node_ref=&scroll style bar_from=300.0>
                        <Frame height=1000.0 />
                    </Scroll>
                </ListItem>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::sized(document, vec2(200.0, 200.0));
    harness.frame(Vec::new());
    let (position, visibility, scroll_to) = shown.get().expect("the bar is built");
    let offset_node = harness
        .document()
        .first_offset_within(scroll)
        .expect("a scroll lays its content out in an offset");
    let scroll_by = |harness: &mut Harness, offset: f32| {
        harness.document_mut().drive_offset(offset_node, offset);
        harness.frame(Vec::new());
    };

    assert_eq!(
        position.get_untracked(),
        ScrollPosition { offset: 0.0, content: 700.0, viewport: 200.0 },
        "the bar reads what lies past where it starts"
    );
    scroll_by(&mut harness, 250.0);
    assert_eq!(position.get_untracked().offset, 0.0);
    assert_eq!(
        visibility.get_untracked(),
        0.0,
        "moving short of where it starts shows no bar"
    );
    scroll_by(&mut harness, 400.0);
    assert_eq!(position.get_untracked().offset, 100.0, "past it, the bar follows");
    assert_eq!(visibility.get_untracked(), 1.0, "and shows");

    with_installed(harness.document_mut(), |_| scroll_to.call(200.0));
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().offset_value(offset_node),
        500.0,
        "a drag on the bar scrolls to its offset past where it starts"
    );
}
