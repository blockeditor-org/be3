use super::*;
use be_sdk_api::{EditorInstanceId, ViewId};

const PLACER: &str = "test.placer";

#[test]
fn an_ask_lands_on_the_view_showing_its_childs_content_and_ends_once_withdrawn() {
    let _root = RootScope::new();
    let placer = EditorInstanceId(1);
    let text = EditorInstanceId(6);
    let (placeholder, content, bar) = (ChildId(2), ChildId(3), ChildId(4));
    // A joined text: its main view placed as the tab's child and shown nowhere, its content in a
    // dock body of its own, and its toolbar in a bar, each a child of the placer's.
    let views = [
        (placeholder, ViewId::MAIN),
        (content, ViewId(1)),
        (bar, ViewId(2)),
    ];
    for (child, view) in views {
        asks::viewed(PLACER, placer, child, (text, view), true);
    }
    let acts = |child: ChildId, view: ViewId| asks::acts(PLACER, placer, child, (text, view, ViewId(1)));

    // The dock gave the bar's tab the keyboard: the content takes it, not the bar.
    asks::heard(PLACER.to_owned(), placer, Some(bar));
    assert!(acts(content, ViewId(1)));
    assert!(!acts(bar, ViewId(2)));
    assert!(!acts(placeholder, ViewId::MAIN));

    // An ask withdrawn (its child no longer placed, or the app reopened) stands no more.
    asks::heard(PLACER.to_owned(), placer, None);
    assert!(!acts(content, ViewId(1)));

    // An ask taken ends too: a view built later does not act on it.
    asks::heard(PLACER.to_owned(), placer, Some(placeholder));
    assert!(acts(content, ViewId(1)));
    asks::taken(PLACER, placer);
    assert!(!acts(content, ViewId(1)));

    // A child not joined, whose content is in its own main view, takes it there.
    let notes = EditorInstanceId(7);
    asks::viewed(PLACER, placer, ChildId(9), (notes, ViewId::MAIN), true);
    asks::heard(PLACER.to_owned(), placer, Some(ChildId(9)));
    assert!(asks::acts(PLACER, placer, ChildId(9), (notes, ViewId::MAIN, ViewId::MAIN)));
}
