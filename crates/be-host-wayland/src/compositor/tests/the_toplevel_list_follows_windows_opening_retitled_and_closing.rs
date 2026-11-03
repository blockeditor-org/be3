use super::*;

fn listed(harness: &Harness) -> Vec<(String, String, bool)> {
    harness
        .client
        .received
        .protocols
        .toplevels
        .iter()
        .map(|listed| (listed.title.clone(), listed.app_id.clone(), listed.closed))
        .collect()
}

fn name(window: &TestWindow, title: &str, app_id: &str) {
    let toplevel = window.toplevel.as_ref().expect("a toplevel");
    toplevel.set_title(title.to_owned());
    toplevel.set_app_id(app_id.to_owned());
    window.surface.commit();
}

#[test]
fn the_toplevel_list_follows_windows_opening_retitled_and_closing() {
    let mut harness = Harness::new();
    let _list = harness.client.toplevel_list();
    let unmapped = harness.client.toplevel();
    harness.settle();
    assert_eq!(listed(&harness), Vec::new(), "a window with nothing drawn is not listed");
    unmapped.destroy();
    harness.settle();

    let (notes, _) = harness.open();
    name(&notes, "Notes", "org.example.Notes");
    harness.settle();
    assert_eq!(
        listed(&harness),
        vec![("Notes".to_owned(), "org.example.Notes".to_owned(), false)]
    );

    let (terminal, _) = harness.open();
    name(&terminal, "Terminal", "org.example.Terminal");
    harness.settle();
    let toplevels = &harness.client.received.protocols.toplevels;
    assert_eq!(toplevels.len(), 2);
    assert!(
        !toplevels[0].identifier.is_empty() && toplevels[0].identifier != toplevels[1].identifier,
        "each window has an identifier of its own"
    );
    let identifier = toplevels[0].identifier.clone();
    let done = toplevels[0].done;

    name(&notes, "Notes — draft", "org.example.Notes");
    harness.settle();
    let toplevels = &harness.client.received.protocols.toplevels;
    assert_eq!(toplevels[0].title, "Notes — draft");
    assert_eq!(toplevels[0].identifier, identifier, "a new title keeps the identifier");
    assert_eq!(toplevels[0].done, done + 1, "the change ends with one done");

    notes.destroy();
    harness.settle();
    assert_eq!(
        listed(&harness),
        vec![
            ("Notes — draft".to_owned(), "org.example.Notes".to_owned(), true),
            ("Terminal".to_owned(), "org.example.Terminal".to_owned(), false),
        ]
    );
}
