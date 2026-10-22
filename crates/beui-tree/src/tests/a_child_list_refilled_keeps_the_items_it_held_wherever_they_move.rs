use super::*;

#[test]
fn a_child_list_refilled_keeps_the_items_it_held_wherever_they_move() {
    let key = |item: &(Option<u64>, &'static str)| item.0;
    let mut list = ChildList::default();
    let slot = list.open();
    list.fill(
        slot,
        vec![(Some(1), "held"), (Some(2), "held"), (None, "held")],
    );
    list.fill_keeping(
        slot,
        vec![
            (Some(3), "new"),
            (Some(1), "new"),
            (None, "new"),
            (Some(2), "new"),
        ],
        key,
    );
    assert_eq!(
        list.iter().copied().collect::<Vec<_>>(),
        [
            (Some(3), "new"),
            (Some(1), "held"),
            (None, "new"),
            (Some(2), "held"),
        ],
        "keyed items keep what was held under their key, and the rest are taken as given"
    );
}
