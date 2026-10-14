use super::*;
use crate::reactive::{build, view};
use crate::styled::{Launcher, LauncherItem};

#[test]
fn a_launcher_query_that_matches_nothing_runs_as_a_command_and_escape_closes() {
    let _root = crate::reactive::RootScope::new();
    let (heard, set_heard) = create_signal(Vec::<String>::new());
    let items = Rc::new(vec![LauncherItem {
        key: "files".to_owned(),
        title: "Files".to_owned(),
        ..LauncherItem::default()
    }]);
    let document = build(move || {
        view! {
            <Launcher
                open=true
                items
                on_launch={move |key: String| set_heard.update(|heard| heard.push(format!("launch {key}")))}
                on_run={move |line: String| set_heard.update(|heard| heard.push(format!("run {line}")))}
                on_close={move || set_heard.update(|heard| heard.push("close".to_owned()))}
            />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    harness.type_text("gtk4-demo --run ");
    harness.frame(Vec::new());
    assert!(
        harness
            .document()
            .find_test_id("launcher.item.files")
            .is_none()
    );
    harness.key(Key::Enter, Modifiers::NONE);
    assert_eq!(heard.get_untracked(), ["run gtk4-demo --run"]);

    harness.key(Key::Escape, Modifiers::NONE);
    assert_eq!(heard.get_untracked(), ["run gtk4-demo --run", "close"]);
}
