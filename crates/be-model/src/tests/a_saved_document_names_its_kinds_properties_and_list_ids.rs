use super::*;
use crate::Stored;

fn entry<'a>(map: &'a Stored, key: &str) -> &'a Stored {
    map.as_map()
        .expect("a map")
        .iter()
        .find(|(name, _)| name.as_text() == Some(key))
        .map(|(_, value)| value)
        .unwrap_or_else(|| panic!("no {key}"))
}

#[test]
fn a_saved_document_names_its_kinds_properties_and_list_ids() {
    let document = board();
    let stored = document.to_stored();

    assert_eq!(entry(&stored, "format"), &Stored::from(1));
    let root = entry(&stored, "root");
    assert_eq!(entry(root, "$kind"), &Stored::from("board"));
    assert_eq!(entry(root, "title"), &Stored::from("Plan"));
    assert_eq!(entry(root, "votes"), &Stored::from(0));
    let columns = entry(root, "columns").as_array().expect("a list");
    let (todo, done, _) = ids(&document);
    assert_eq!(entry(&columns[0], "$kind"), &Stored::from("column"));
    assert_eq!(
        entry(&columns[0], "$id"),
        &Stored::Tag(
            37,
            Box::new(Stored::Bytes(todo.as_uuid().as_bytes().to_vec()))
        )
    );
    assert_eq!(
        entry(&columns[1], "$id"),
        &Stored::Tag(
            37,
            Box::new(Stored::Bytes(done.as_uuid().as_bytes().to_vec()))
        )
    );

    let reloaded = Document::<Board>::from_bytes(&document.to_bytes()).expect("the bytes decode");
    assert_eq!(ids(&reloaded), ids(&document));
    assert_eq!(reloaded, document);
}
