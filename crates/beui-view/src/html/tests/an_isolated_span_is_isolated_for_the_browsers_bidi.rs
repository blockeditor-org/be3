use super::*;

/// The HTML spec's example for `<bdi>`: without isolation, ": 3" after a right-to-left name
/// joins its run and reads "User 3 :<name> posts".
fn posts(isolate: bool) -> NodeId {
    view! {
        <Text color={INK}>
            "User "
            <Span isolate={isolate}>"\u{5e9}\u{5dc}\u{5d5}\u{5dd}"</Span>
            ": 3 posts"
        </Text>
    }
}

#[test]
fn an_isolated_span_is_isolated_for_the_browsers_bidi() {
    let scope = ::beui_reactive::Scope::detached();
    let (isolated, text) = scope.run(|| {
        let isolated = create_signal(true);
        let text = view! {
            <Text color={INK}>
                "Leave \u{201c}"
                <Span isolate={isolated}>"Team"</Span>
                "\u{201d}?"
            </Text>
        };
        (isolated, text)
    });
    ::beui_reactive::flush();
    let markup = || beui_core_html::document::element(text).unwrap().to_html();
    let before = markup();
    assert!(before.contains("dir=\"auto\""), "{before}");
    assert!(before.contains("unicode-bidi: isolate"), "{before}");
    isolated.set(false);
    ::beui_reactive::flush();
    let after = markup();
    assert!(!after.contains("dir="), "{after}");
    assert!(!after.contains("unicode-bidi"), "{after}");
    scope.dispose();

    painting([320, 80], || {
        view! {
            <Root clear_color={PAPER}>
                <List spacing={12.0}>{posts(false)}{posts(true)}</List>
            </Root>
        }
    })
    .snapshot("html_text_isolate.browser");
}
