use super::*;
use crate::Stored;

#[test]
fn saving_the_same_document_twice_gives_the_same_parts() {
    let document = deck(3000);
    let parts = document.to_parts();
    assert_eq!(document.to_parts(), parts);
    assert_eq!(from_parts::<Deck>(&parts).to_parts(), parts);
    assert_eq!(
        Document::<Deck>::from_bytes(&document.to_bytes())
            .expect("the bytes decode")
            .to_parts(),
        parts
    );

    let kept = |entries: Vec<(&str, i32)>| {
        let unknown = Stored::Map(
            entries
                .into_iter()
                .map(|(key, value)| (Stored::from(key), Stored::from(value)))
                .collect(),
        );
        let root = Stored::Map(vec![
            (Stored::from("$kind"), Stored::from("deck")),
            (Stored::from("newer"), unknown),
        ]);
        let stored = Stored::Map(vec![
            (Stored::from("format"), Stored::from(1)),
            (Stored::from("root"), root),
        ]);
        Document::<Deck>::from_bytes(&crate::to_cbor(&stored))
            .expect("the bytes decode")
            .to_parts()
    };
    assert_eq!(
        kept(vec![("bb", 1), ("a", 2)]),
        kept(vec![("a", 2), ("bb", 1)])
    );
}
