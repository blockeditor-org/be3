use super::*;

#[test]
fn focus_is_replayed_once_for_each_change() {
    let mut replay = Replay::default();
    assert!(!replay.focused());
    assert_eq!(
        replayed(
            &mut replay,
            vec![
                Forwarded::Focus(true),
                Forwarded::Input(Event::Focus(true)),
                Forwarded::Focus(true),
            ],
            1.0
        ),
        [Input::Focus(true)]
    );
    assert!(replay.focused());
    assert_eq!(
        replayed(
            &mut replay,
            vec![
                Forwarded::Input(Event::Focus(false)),
                Forwarded::Focus(false),
            ],
            1.0
        ),
        [Input::Focus(false)]
    );
    assert!(!replay.focused());
    assert_eq!(
        replayed(&mut replay, vec![Forwarded::Focus(false)], 1.0),
        []
    );
}
