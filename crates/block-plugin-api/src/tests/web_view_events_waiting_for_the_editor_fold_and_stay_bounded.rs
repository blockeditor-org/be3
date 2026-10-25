use super::*;

#[test]
fn web_view_events_waiting_for_the_editor_fold_and_stay_bounded() {
    let mut events = Vec::new();
    for event in [
        WebViewEvent::Navigate("a".into()),
        WebViewEvent::Title("A".into()),
        WebViewEvent::Title("A, loaded".into()),
        WebViewEvent::Push("b".into()),
        WebViewEvent::Title("B".into()),
    ] {
        WebViewEvent::queue(&mut events, event);
    }
    assert_eq!(
        events,
        [
            WebViewEvent::Navigate("a".into()),
            WebViewEvent::Title("A, loaded".into()),
            WebViewEvent::Push("b".into()),
            WebViewEvent::Title("B".into()),
        ],
        "a title right after a title replaces it; one after another event waits"
    );

    for page in 0..WAITING_WEB_VIEW_EVENTS * 2 {
        WebViewEvent::queue(&mut events, WebViewEvent::Push(page.to_string()));
    }
    assert_eq!(events.len(), WAITING_WEB_VIEW_EVENTS);
    assert_eq!(
        events.last(),
        Some(&WebViewEvent::Push((WAITING_WEB_VIEW_EVENTS * 2 - 1).to_string())),
        "the newest events stay"
    );
}
