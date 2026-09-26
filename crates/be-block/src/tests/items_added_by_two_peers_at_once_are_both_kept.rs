use super::*;

#[test]
fn items_added_by_two_peers_at_once_are_both_kept() {
    let base = edited(&ChecklistContent::default(), [Checklist::add("milk").1]);
    let (_, ours) = Checklist::add("eggs");
    let (_, theirs) = Checklist::add("bread");

    for sequenced in [
        edited(&base, [ours.clone(), theirs.clone()]),
        edited(&base, [theirs.clone(), ours.clone()]),
    ] {
        let mut texts: Vec<String> = sequenced
            .root()
            .items
            .iter()
            .map(|item| item.text.clone())
            .collect();
        assert_eq!(texts[0], "milk");
        texts.sort();
        assert_eq!(texts, ["bread", "eggs", "milk"]);
    }
}
