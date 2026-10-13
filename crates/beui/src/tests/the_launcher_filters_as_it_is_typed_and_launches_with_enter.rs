use super::*;
use crate::reactive::{build, create_signal, view};
use crate::styled::{Launcher, LauncherItem};

fn item(key: &str, title: &str, terms: &[&str]) -> LauncherItem {
    LauncherItem {
        key: key.to_owned(),
        title: title.to_owned(),
        detail: String::new(),
        terms: terms.iter().map(|term| (*term).to_owned()).collect(),
        image: None,
    }
}

#[test]
fn the_launcher_filters_as_it_is_typed_and_launches_with_enter() {
    let heard = Rc::new(RefCell::new(Vec::<String>::new()));
    let (launched, ran, closed) = (heard.clone(), heard.clone(), heard.clone());
    let items = Rc::new(vec![
        item("files", "Files", &["folder"]),
        item("foot", "Foot", &["terminal"]),
        item("term", "Terminal", &["shell"]),
    ]);
    let document = build(move || {
        let (open, set_open) = create_signal(true);
        let (closing, launching, running) = (set_open.clone(), set_open.clone(), set_open);
        view! {
            <Launcher
                open
                items
                on_launch={move |key: String| {
                    launched.borrow_mut().push(format!("launch {key}"));
                    launching.set(false);
                }}
                on_run={move |line: String| {
                    ran.borrow_mut().push(format!("run {line}"));
                    running.set(false);
                }}
                on_close={move || {
                    closed.borrow_mut().push("close".to_owned());
                    closing.set(false);
                }}
            />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let shown = |harness: &Harness| {
        ["files", "foot", "term"]
            .into_iter()
            .filter(|key| {
                harness
                    .document()
                    .find_test_id(&format!("launcher.item.{key}"))
                    .is_some()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(shown(&harness), ["files", "foot", "term"]);
    assert!(harness.document().find_test_id("launcher.run").is_none());

    harness.type_text("term");
    harness.frame(Vec::new());
    assert_eq!(shown(&harness), ["foot", "term"]);
    assert!(
        harness.document().find_test_id("launcher.run").is_some(),
        "what was typed can be run as a command"
    );
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::ArrowUp, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    assert_eq!(
        *heard.borrow(),
        ["launch foot"],
        "the name ranks first, and the arrows move the highlight"
    );
}
