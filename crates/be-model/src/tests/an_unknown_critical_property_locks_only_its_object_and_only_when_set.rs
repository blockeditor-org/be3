use super::*;

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "folder")]
struct Folder {
    name: String,
    items: List<Item>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "item")]
struct Item {
    name: String,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "item")]
struct ClippedItem {
    name: String,
    #[critical]
    clip: bool,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "folder")]
struct NewerFolder {
    name: String,
    items: List<ClippedItem>,
}

fn item(name: &str, clip: bool) -> ClippedItem {
    ClippedItem {
        name: name.to_owned(),
        clip,
    }
}

#[test]
fn an_unknown_critical_property_locks_only_its_object_and_only_when_set() {
    let newer = Document::new(&NewerFolder {
        name: "both".to_owned(),
        items: [item("plain", false), item("clipped", true)]
            .into_iter()
            .collect(),
    });
    let ids: Vec<ObjectId> = newer.root().items.iter().map(|held| held.id).collect();

    let mut older = Document::<Folder>::from_bytes(&newer.to_bytes()).expect("the bytes decode");
    assert!(!older.is_locked(ObjectId::ROOT));
    assert!(!older.is_locked(ids[0]));
    assert!(older.is_locked(ids[1]));

    let renamed = Edit(vec![
        Item::NAME.set(ids[0], &"renamed".to_owned()),
        Item::NAME.set(ids[1], &"renamed".to_owned()),
    ]);
    assert!(
        older
            .step(&Edit(vec![Item::NAME.set(ids[1], &"renamed".to_owned())]))
            .is_none()
    );
    older.apply(&renamed);
    let names: Vec<String> = older
        .root()
        .items
        .iter()
        .map(|held| held.name.clone())
        .collect();
    assert_eq!(names, ["renamed", "clipped"]);

    let back = Document::<NewerFolder>::from_bytes(&older.to_bytes()).expect("the bytes decode");
    assert_eq!(
        back.root()
            .items
            .iter()
            .map(|held| held.value.clone())
            .collect::<Vec<_>>(),
        [item("renamed", false), item("clipped", true)]
    );

    older.apply(&Change::remove(ids[1]).into());
    assert_eq!(older.root().items.len(), 1);
}
