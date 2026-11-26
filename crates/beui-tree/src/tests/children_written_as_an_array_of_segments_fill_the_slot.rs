use super::*;
use crate::reactive::into_segment;

/// A children slot takes an array of segments written by hand, as it takes one segment.
#[test]
fn children_written_as_an_array_of_segments_fill_the_slot() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let page = scope.run(|| {
        let first = view! {
            <Element tag="p" text="first" />
        };
        let second = view! {
            <Element tag="span" text="second" />
        };
        view! {
            <Element tag="div" children={[into_segment(first), into_segment(second)]} />
        }
    });
    assert!(page.html().contains(">first</p>"));
    assert!(page.html().contains(">second</span>"));
    assert_eq!(page.0.children.borrow().len(), 2);
}
