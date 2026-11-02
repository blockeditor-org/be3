use super::*;
use crate::selection::client;

#[test]
fn a_file_dropped_from_outside_reaches_the_window_under_the_pointer() {
    let mut harness = Harness::new();
    let _pointer = harness.client.pointer();
    let _device = harness.client.data_device();
    let (_beside, _) = harness.open();
    let (onto, second) = harness.open();
    let end = harness.window(second).center();
    let over = format!("{},{}", end.x, end.y - 10.0);
    let at = format!("{},{}", end.x, end.y);

    harness
        .stepped
        .ask(&["move", "2,2"])
        .expect("the pointer moves");
    harness
        .stepped
        .ask(&["grab", "/home/me/a note.txt", "/home/me/b.txt"])
        .expect("the files are grabbed");
    harness.settle();
    assert_eq!(
        client::drop_target(),
        None,
        "no window is under the pointer"
    );

    harness
        .stepped
        .ask(&["move", &over])
        .expect("the files are carried");
    harness.settle();
    assert_eq!(
        client::drop_target().as_ref(),
        Some(&onto.surface),
        "the drag enters the window the files are carried over"
    );
    let offer = client::dragged().expect("the drag offers its files");
    assert!(client::mime_types(&offer).contains(&"text/uri-list".to_owned()));

    harness
        .stepped
        .ask(&["drop", &at])
        .expect("the files are dropped");
    harness.settle();
    assert_eq!(client::dropped_on().as_ref(), Some(&onto.surface));
    let uris = client::ask_as(&offer, "text/uri-list");
    let paths = client::ask(&offer);
    harness.settle();
    assert_eq!(
        client::answer(uris),
        "file:///home/me/a%20note.txt\r\nfile:///home/me/b.txt\r\n"
    );
    assert_eq!(client::answer(paths), "/home/me/a note.txt\n/home/me/b.txt");
}
