use super::*;
use crate::native::testing::Showing;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_slider_asks_once_for_a_value_its_owner_has_not_shown_yet() {
    let _root = RootScope::new();
    let asked = create_signal(Vec::new());
    let shown = create_signal(0.1);
    let document = build_node(move || {
        view! {
            <Slider
                @test_id={"volume"}
                value={Binding::new(shown, move |next| asked.update(|asked| asked.push(next)))}
                label="Volume"
            />
        }
    });
    let mut app = headless(Showing(document));

    app.click("volume");
    assert_eq!(
        asked.get_untracked().len(),
        1,
        "a quick click asks once: {:?}",
        asked.get_untracked()
    );

    asked.set(Vec::new());
    app.run(&["down", "#volume+10,5"]);
    app.run(&["pause", "100"]);
    app.run(&["up"]);
    assert_eq!(
        asked.get_untracked().len(),
        1,
        "a slow click asks once too, though its press and its drag both land on the same value: {:?}",
        asked.get_untracked()
    );
}
