use super::*;
use crate::reactive::{ErrorBoundary, Errored, Pending, create_source};

#[test]
fn a_mock_dom_errored_shows_its_fallback_while_what_its_content_read_has_failed() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let name = create_source::<String>();
    name.set(Ok("Groceries".to_owned()));
    let page = scope.run(|| {
        let fallback = |(error, _reset): (String, ErrorBoundary)| {
            view! {
                <Element tag="p" text={error} />
            }
        };
        view! {
            <Element tag="div">
                <Errored fallback>|| {
                    let shown = create_memo(move || name.get().unwrap_or_default());
                    view! {
                        <Element tag="h1" text={shown} />
                    }
                }</Errored>
            </Element>
        }
    });
    assert_eq!(page.html(), "<div id=0><h1 id=1>Groceries</h1></div>");

    name.set(Err(Pending::Failed("refused".to_owned())));
    flush();
    assert_eq!(page.html(), "<div id=0><p id=2>refused</p></div>");

    name.set(Ok("Errands".to_owned()));
    flush();
    assert_eq!(
        page.html(),
        "<div id=0><h1 id=1>Errands</h1></div>",
        "the content it kept shows again once nothing it read has failed"
    );
}
