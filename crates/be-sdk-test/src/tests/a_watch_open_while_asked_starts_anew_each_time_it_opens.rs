use beui::styled::Root;

use be_sdk_beui::{CapturedChord, CapturedTrigger, InputCapture, InputCaptureWatch};

use super::*;

struct CaptureRoot;

#[component]
impl BeuiRoot for CaptureRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Capture editor={editor} />
            </Root>
        }
    }
}

#[component]
fn Capture(editor: Editor) -> NodeId {
    let open = create_signal(false);
    let capture = editor.watch_while::<InputCaptureWatch>(move || open.get());
    let text = create_memo(move || {
        capture.with(|capture| format!("{:?} {}", capture.held, capture.chord.is_some()))
    });
    view! {
        <List spacing=Space::Sm>
            <Button
                label="Capture"
                @test_id={"capture.toggle"}
                on_click={move || open.update(|open| *open = !*open)}
            />
            <Text @test_id={"capture.seen"}>{text}</Text>
        </List>
    }
}

#[test]
fn a_watch_open_while_asked_starts_anew_each_time_it_opens() {
    let mut test: BeuiTest<CaptureRoot> =
        BeuiTest::new(OwnedEditor::new(EditorHost::default(), Uuid::new_v4()));
    test.run();
    assert!(!test.watches::<InputCaptureWatch>(), "closed until asked");

    test.click("capture.toggle");
    test.run();
    assert!(test.watches::<InputCaptureWatch>());
    test.set_watched::<InputCaptureWatch>(&InputCapture {
        held: vec![29],
        chord: Some(CapturedChord {
            held: vec![29],
            trigger: CapturedTrigger::Source(46),
        }),
    });
    test.run();
    assert_eq!(test.label("capture.seen"), "[29] true");

    test.click("capture.toggle");
    test.run();
    assert!(
        !test.watches::<InputCaptureWatch>(),
        "closed once not asked"
    );
    assert_eq!(test.label("capture.seen"), "[29] true", "kept once closed");

    test.click("capture.toggle");
    test.run();
    assert!(test.watches::<InputCaptureWatch>(), "a new watch opens");
    assert_eq!(
        test.label("capture.seen"),
        "[] false",
        "a new watch hears nothing of the one before"
    );
}
