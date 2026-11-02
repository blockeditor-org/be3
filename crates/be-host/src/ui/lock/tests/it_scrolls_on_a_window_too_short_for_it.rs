use super::*;
use beui_host::native::build;

#[test]
fn it_scrolls_on_a_window_too_short_for_it() {
    let _root = RootScope::new();
    let document = build(move || {
        view! {
            <Root>
                <LockScreen
                    open=true
                    time="09:41"
                    date="Thursday, 8 October"
                    user="Ada Lovelace"
                    error={Some("That password is not right.".to_owned())}
                    actions={vec![suspend()]}
                    on_submit={|_: String| ()}
                />
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, Vec2::new(320.0, 180.0));
    test.frame(Vec::new());
    test.snapshot("lock_screen_on_a_short_window");
    assert!(
        test.rect_of("lock.action.0").bottom() > 180.0,
        "the card is taller than the window"
    );
    test.reveal("lock.action.0");
    assert!(
        test.rect_of("lock.action.0").bottom() <= 180.0,
        "and scrolls to show what lies below"
    );
}
