use super::*;

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Log {
    #[model(merge_appends)]
    lines: List<Card>,
    cards: List<Card>,
}

fn texts(cards: &List<Card>) -> Vec<&str> {
    cards.iter().map(|card| card.text.as_str()).collect()
}

fn appended(document: &Document<Log>, anchor: Anchor, texts: &[&str]) -> Document<Log> {
    let mut document = document.clone();
    for text in texts {
        document.apply(
            &Log::LINES
                .insert(ObjectId::ROOT, anchor, &card(text))
                .1
                .into(),
        );
        document.apply(
            &Log::CARDS
                .insert(ObjectId::ROOT, anchor, &card(text))
                .1
                .into(),
        );
    }
    document
}

#[test]
fn a_list_that_merges_appends_keeps_the_published_side_and_puts_ours_after_it() {
    let base = appended(&Document::default(), Anchor::End, &["a", "b"]);
    let first = base.root().lines[0].id;
    let first_card = base.root().cards[0].id;
    let early = |document: &mut Document<Log>, text: &str| {
        document.apply(
            &[
                Log::LINES
                    .insert(ObjectId::ROOT, Anchor::After(first), &card(text))
                    .1,
                Log::CARDS
                    .insert(ObjectId::ROOT, Anchor::After(first_card), &card(text))
                    .1,
            ]
            .into_iter()
            .collect(),
        );
    };
    let mut ours = base.clone();
    early(&mut ours, "ours early");
    let ours = appended(&ours, Anchor::End, &["ours 1", "ours 2"]);
    let mut theirs = appended(&base, Anchor::End, &["theirs 1"]);
    early(&mut theirs, "theirs early");

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    assert_eq!(
        texts(&merged.root().lines),
        [
            "a",
            "theirs early",
            "b",
            "theirs 1",
            "ours early",
            "ours 1",
            "ours 2"
        ],
        "the published side keeps its order and ours follows all of it"
    );
    assert_eq!(
        texts(&merged.root().cards),
        [
            "a",
            "ours early",
            "theirs early",
            "b",
            "ours 1",
            "ours 2",
            "theirs 1"
        ],
        "any other list puts an insert after the sibling it was made after"
    );
}
