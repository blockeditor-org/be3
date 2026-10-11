use zbus::zvariant::ObjectPath;

use super::*;

#[test]
fn an_item_reads_its_properties() {
    let pixmaps = vec![(1_i32, 1_i32, vec![0xff_u8, 1, 2, 3])];
    let tooltip = Value::from((
        "",
        Vec::<(i32, i32, Vec<u8>)>::new(),
        "",
        "<b>3</b> unread &amp; waiting",
    ));
    let map: HashMap<String, OwnedValue> = [
        ("Id", Value::from("chat")),
        ("Title", Value::from("Chat")),
        ("Status", Value::from("NeedsAttention")),
        ("IconName", Value::from("chat-symbolic")),
        ("IconPixmap", Value::from(pixmaps)),
        ("ToolTip", tooltip),
        ("ItemIsMenu", Value::from(true)),
        (
            "Menu",
            Value::from(ObjectPath::try_from("/MenuBar").expect("a path")),
        ),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), owned(value)))
    .collect();
    let read = properties(&map);
    assert_eq!(read.id, "chat");
    assert_eq!(read.title, "Chat");
    assert_eq!(read.status, TrayStatus::NeedsAttention);
    assert_eq!(read.icon_name, "chat-symbolic");
    assert_eq!(
        read.icon_pixmaps,
        vec![Pixmap {
            width: 1,
            height: 1,
            argb: vec![0xff, 1, 2, 3],
        }]
    );
    assert_eq!(read.tooltip, "3 unread & waiting", "markup is dropped");
    assert!(read.item_is_menu);
    assert_eq!(read.menu.as_deref(), Some("/MenuBar"));

    let bare = properties(&HashMap::from([(
        "Menu".to_owned(),
        owned(Value::from(ObjectPath::try_from("/").expect("a path"))),
    )]));
    assert_eq!(
        bare.status,
        TrayStatus::Active,
        "an item is active unless it says"
    );
    assert_eq!(bare.menu, None, "the root path is no menu");
}
