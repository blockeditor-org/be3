use super::*;
use crate::{Items, Objects, Place, Tree, Value};

#[test]
fn a_card_saved_among_columns_by_an_older_build_reads_as_a_column() {
    let document = board();
    let (todo, _, write) = ids(&document);
    let mut objects = document.tree().objects().clone();
    let listing = Board::COLUMNS.of(ObjectId::ROOT);
    let listed = |objects: &Objects, place: Place| match &objects[&place.object].fields
        [usize::from(place.field)]
    {
        Value::List(items) => items.ids(),
        _ => Vec::new(),
    };
    let cards = Column::CARDS.of(todo);
    let left = listed(&objects, cards)
        .into_iter()
        .filter(|id| *id != write);
    objects.get_mut(&todo).expect("todo").fields[usize::from(cards.field)] =
        Value::List(Items::from_ids(left));
    let mut among = listed(&objects, listing);
    among.push(write);
    objects.get_mut(&ObjectId::ROOT).expect("the root").fields[usize::from(listing.field)] =
        Value::List(Items::from_ids(among));
    objects.get_mut(&write).expect("write").parent = Some(listing);
    let saved = Document::<Board>::from_tree(Tree::from_objects(objects));

    let reloaded = Document::<Board>::from_bytes(&saved.to_bytes()).expect("the bytes decode");

    assert_eq!(
        columns(&reloaded),
        owned(&[("Todo", &["review"]), ("Done", &[]), ("write", &[])])
    );
    let again = Document::<Board>::from_bytes(&reloaded.to_bytes()).expect("the bytes decode");
    assert_eq!(again, reloaded);
}
