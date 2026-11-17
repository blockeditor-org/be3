use be_sdk_api::{AdministratorWatch, Watched, ops::guest::with_parent};

use super::a_watch_of_an_extension_the_manifest_does_not_grant_is_denied::{exchange, opened};
use super::*;

#[test]
fn closing_the_last_watch_of_a_kind_unwatches_it() {
    let mut wire = Wire::default();
    let (guest, editor, mut heard) = opened(&mut wire);
    let first = guest.enter(|| with_parent(editor, AdministratorWatch::open));
    let second = guest.enter(|| with_parent(editor, AdministratorWatch::open));
    exchange(&mut wire, &guest, &mut heard);
    heard.clear();

    guest.enter(|| drop(first));
    exchange(&mut wire, &guest, &mut heard);
    assert!(
        heard.is_empty(),
        "another watch of the kind is still open: {heard:?}"
    );

    guest.enter(|| drop(second));
    exchange(&mut wire, &guest, &mut heard);
    assert!(
        matches!(heard.as_slice(), [Heard::Unwatch(_, open)] if *open == AdministratorWatch::OPEN),
        "{heard:?}"
    );
}
