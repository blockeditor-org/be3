use super::*;
use crate::Stored;

#[test]
fn list_ids_survive_saving_and_a_repeated_id_gets_a_fresh_one() {
    let document = board();
    let (todo, _, write) = ids(&document);
    let mut stored = document.to_stored();
    let Stored::Map(entries) = &mut stored else {
        panic!("a document is a map");
    };
    let root = &mut entries
        .iter_mut()
        .find(|(key, _)| key.as_text() == Some("root"))
        .expect("a root")
        .1;
    let Stored::Map(root) = root else {
        panic!("the root is a map");
    };
    let columns = &mut root
        .iter_mut()
        .find(|(key, _)| key.as_text() == Some("columns"))
        .expect("columns")
        .1;
    let Stored::Array(columns) = columns else {
        panic!("columns are a list");
    };
    let copy = columns[0].clone();
    columns.push(copy);

    let reloaded =
        Document::<Board>::from_bytes(&crate::stored::encode(&stored)).expect("the bytes decode");
    let reloaded_ids: Vec<ObjectId> = reloaded.root().columns.iter().map(|held| held.id).collect();
    assert_eq!(reloaded_ids.len(), 3);
    assert_eq!(reloaded_ids[0], todo);
    assert_ne!(reloaded_ids[2], todo);
    assert_eq!(reloaded.root().columns[0].cards[0].id, write);
    assert_ne!(reloaded.root().columns[2].cards[0].id, write);
    assert_eq!(
        columns_of(&reloaded),
        owned(&[
            ("Todo", &["write", "review"]),
            ("Done", &[]),
            ("Todo", &["write", "review"])
        ])
    );
}

fn columns_of(document: &Document<Board>) -> Vec<(String, Vec<String>)> {
    columns(document)
}
