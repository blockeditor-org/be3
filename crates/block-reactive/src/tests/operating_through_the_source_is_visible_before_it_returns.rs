use super::*;

#[test]
fn operating_through_the_source_is_visible_before_it_returns() {
    let client = client();
    let source = BlockSource::new(client.create_block(Calendar::default()), || {});
    let events = source.project(|calendar: &Calendar| calendar.events().len());

    source.operate(add_event("write"));
    assert_eq!(events.get_untracked(), 1);
    source.operate(add_event("review"));
    assert_eq!(events.get_untracked(), 2);
}
