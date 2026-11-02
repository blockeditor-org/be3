use super::*;
use zbus::zvariant::Str;

#[test]
fn hints_say_how_urgent_a_notification_is_and_what_it_shows() {
    let pixels = vec![255u8, 0, 0, 0, 255, 0, 9, 9];
    let image = Value::from((2i32, 1i32, 8i32, false, 8i32, 3i32, pixels));
    let hints: HashMap<String, OwnedValue> = [
        ("urgency", OwnedValue::from(2u8)),
        ("transient", OwnedValue::from(true)),
        (
            "desktop-entry",
            OwnedValue::from(Str::from("org.example.Mail")),
        ),
        ("image-data", OwnedValue::try_from(image).unwrap()),
        ("image-path", OwnedValue::from(Str::from("/tmp/cat.png"))),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value))
    .collect();
    let read = read(&hints);
    assert_eq!(read.urgency, Urgency::Critical);
    assert!(read.transient);
    assert!(!read.resident);
    assert_eq!(read.desktop_entry.as_deref(), Some("org.example.Mail"));
    assert_eq!(read.image_path.as_deref(), Some("/tmp/cat.png"));
    let image = read.image.expect("the image data is read");
    assert_eq!((image.width(), image.height()), (2, 1));
    assert_eq!(
        image.pixels(),
        &[255, 0, 0, 255, 0, 255, 0, 255],
        "rows are read at their stride and given full alpha"
    );

    let none = read_none();
    assert_eq!(none.urgency, Urgency::Normal);
    assert!(none.image.is_none());
}

fn read_none() -> Hints {
    read(&HashMap::new())
}
