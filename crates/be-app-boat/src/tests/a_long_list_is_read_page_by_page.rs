use super::*;

#[test]
fn a_long_list_is_read_page_by_page() {
    let mut boat = Boat::signed_in();

    let first = boat.fetch();
    assert_eq!(first.request.url, LIST);
    boat.answer(&first, 200, FIRST_PAGE);
    let last = boat.fetch();
    assert_eq!(
        last.request.url,
        format!("{LIST}&cursor=MjAyNi0wMi0yMFQxMDowMjozMS44ODBaOmJ4X3I0dDh3M25l")
    );
    boat.answer(&last, 200, LAST_PAGE);

    assert_eq!(boat.kept().map(|kept| kept.sandboxes.len()), Some(3));
    assert!(boat.test.shown("boat.sandbox.bx_h6j9c2vd.detail"));
}
