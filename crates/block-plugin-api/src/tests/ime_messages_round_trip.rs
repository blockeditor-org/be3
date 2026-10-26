use super::*;

#[test]
fn ime_messages_round_trip() {
    round_trips(&FrameCommit {
        windows: vec![ScreenWindow {
            screen: ScreenId(6),
            window: WindowRequests {
                ime: Some(ImeArea {
                    rect: Rect::from_min_size(Pos2::new(1.0, 2.0), Vec2::new(3.0, 4.0)),
                    cursor: Rect::from_min_size(Pos2::new(5.0, 6.0), Vec2::new(1.0, 12.0)),
                    text: Some(ImeText {
                        start: 4,
                        text: "say helo".into(),
                        selection: 8..8,
                        composing: Some(4..8),
                    }),
                    keyboard: Keyboard::Shown { asks: 3 },
                }),
                ..WindowRequests::default()
            },
        }],
        ..frame(1)
    });
    round_trips(&InputBatch {
        screen: ScreenId(2),
        every_sample: false,
        events: vec![
            Input::Ime(ImeEvent::Enabled).into(),
            Input::Ime(ImeEvent::SetComposingText("か".into())).into(),
            Input::Ime(ImeEvent::CommitText("漢".into())).into(),
            Input::Ime(ImeEvent::FinishComposing).into(),
            Input::Ime(ImeEvent::SetComposingRegion(4..8)).into(),
            Input::Ime(ImeEvent::ReplaceText {
                range: 4..8,
                text: "hello".into(),
            })
            .into(),
            Input::Ime(ImeEvent::DeleteSurrounding {
                before: 1,
                after: 2,
            })
            .into(),
            Input::Ime(ImeEvent::SetSelection {
                anchor: 3,
                focus: 3,
            })
            .into(),
            Input::Ime(ImeEvent::Disabled).into(),
        ],
    });
}
