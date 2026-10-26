use super::*;
use crate::KeyAt;
use crate::KeyChord;
use crate::reactive::*;

#[test]
fn a_tap_intercepted_for_a_document_runs_its_tap_action() {
    let _root = crate::reactive::RootScope::new();
    let runs = create_signal(0);
    let document = build_node(move || {
        Action::new("test.launcher", "Programs", move || {
            runs.update(|runs| *runs += 1)
        })
        .shortcut(Chord::tap(Key::Logo))
        .at(KeyAt::Window)
        .capture()
        .register();
        view! {
            <List spacing=0.0 />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .intercepted_keys,
        [KeyChord::tap(Key::Logo)],
        "an intercepting tap is named as a tap"
    );
    harness.frame(vec![Input::InterceptedTap(Key::Logo)]);
    assert_eq!(
        runs.get_untracked(),
        1,
        "a tap intercepted for the document runs it"
    );
    harness.frame(vec![Input::InterceptedTap(Key::Alt)]);
    assert_eq!(runs.get_untracked(), 1, "a tap of another key does not");
}
