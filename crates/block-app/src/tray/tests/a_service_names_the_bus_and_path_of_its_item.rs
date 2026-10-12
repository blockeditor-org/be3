use super::*;

#[test]
fn a_service_names_the_bus_and_path_of_its_item() {
    let named = Address::registered("org.kde.StatusNotifierItem-4077-1", Some(":1.42"))
        .expect("a bus name is a service");
    assert_eq!(named.bus, "org.kde.StatusNotifierItem-4077-1");
    assert_eq!(
        named.path, ITEM_PATH,
        "a bare bus name serves the default path"
    );

    let pathed = Address::registered("/org/ayatana/NotificationItem/chat", Some(":1.7"))
        .expect("a path is a service of its sender");
    assert_eq!(pathed.bus, ":1.7");
    assert_eq!(pathed.path, "/org/ayatana/NotificationItem/chat");
    assert_eq!(pathed.key(), ":1.7/org/ayatana/NotificationItem/chat");
    assert_eq!(
        Address::listed(&pathed.key()),
        Some(pathed),
        "what the watcher lists names the item it was registered as"
    );

    assert_eq!(Address::registered("/StatusNotifierItem", None), None);
    assert_eq!(Address::listed(""), None);
    assert_eq!(Address::listed("/StatusNotifierItem"), None);
}
