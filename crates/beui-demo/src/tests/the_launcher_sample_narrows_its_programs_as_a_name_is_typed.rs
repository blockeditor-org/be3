use super::*;
use beui::Key;

#[test]
fn the_launcher_sample_narrows_its_programs_as_a_name_is_typed() {
    let mut test = alone(WIDE, Page::Overlays);
    test.click("demo.launcher.open");
    test.frame(Vec::new());
    assert!(test.shown("launcher.item.files"));
    assert!(test.shown("launcher.item.terminal"));
    test.snapshot("launcher");

    test.frame(vec![Input::Text("te".to_owned())]);
    test.frame(Vec::new());
    assert!(test.shown("launcher.item.text"));
    assert!(test.shown("launcher.item.terminal"));
    assert!(!test.shown("launcher.item.files"));
    test.snapshot("launcher_filtered");

    test.frame(
        [true, false]
            .into_iter()
            .map(|pressed| {
                Input::Key(beui::Keystroke {
                    key: Some(Key::Enter),
                    pressed,
                    ..Default::default()
                })
            })
            .collect(),
    );
    test.frame(Vec::new());
    assert!(!test.shown("launcher"), "launching closes the launcher");
}
