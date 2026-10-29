use super::*;

fn read(items: &mut Items, key: &str, title: &str) {
    let item = items.get_mut(key).expect("the item was added");
    item.properties = Some(Properties {
        id: title.to_lowercase(),
        title: title.to_owned(),
        menu: Some("/Menu".to_owned()),
        ..Properties::default()
    });
}

#[test]
fn items_are_published_once_read_and_go_with_their_owner() {
    let mut items = Items::default();
    let chat = Address::registered("/Chat", Some(":1.4")).expect("an address");
    let sync = Address::listed("org.example.Sync").expect("an address");
    assert!(items.add(chat.clone(), ":1.4".to_owned()));
    assert!(items.add(sync.clone(), String::new()));
    assert!(
        !items.add(chat.clone(), ":1.4".to_owned()),
        "registering again adds nothing"
    );
    assert!(
        items.published().is_empty(),
        "nothing shows before its properties are read"
    );

    read(&mut items, &chat.key(), "Chat");
    read(&mut items, &sync.key(), "Sync");
    items.get_mut(&sync.key()).expect("listed").owner = ":1.9".to_owned();
    let published = items.published();
    assert_eq!(
        published
            .iter()
            .map(|item| (item.id.as_str(), item.title.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (":1.4/Chat", "Chat"),
            ("org.example.Sync/StatusNotifierItem", "Sync")
        ],
        "in the order they came"
    );

    assert_eq!(items.signalled(":1.4", "/Chat"), vec![chat.key()]);
    assert!(items.signalled(":1.4", "/Other").is_empty());
    assert_eq!(items.menu_signalled(":1.9", "/Menu"), vec![sync.key()]);

    assert_eq!(
        items.vanished(":1.9"),
        vec![sync.key()],
        "a well-known name goes with its owner"
    );
    assert_eq!(items.vanished(":1.4"), vec![chat.key()]);
    assert!(items.published().is_empty());
}
