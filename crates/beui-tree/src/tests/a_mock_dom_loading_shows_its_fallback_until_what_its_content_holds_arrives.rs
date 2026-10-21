use super::*;
use crate::reactive::{Loading, Pending, Ready};

#[test]
fn a_mock_dom_loading_shows_its_fallback_until_what_its_content_holds_arrives() {
    let _root = ::reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let name = create_signal::<Ready<String>>(Err(Pending::Loading));
    let effects = store(Cell::new(0));
    let page = scope.run(|| {
        let fallback = |pending: Pending| match pending {
            Pending::Loading => view! { <Element tag="p" text="loading" /> },
            Pending::Failed(error) => view! { <Element tag="p" text={error} /> },
        };
        view! {
            <Element tag="div">
                <Loading fallback>|| {
                    create_effect(move || effects.with(|effects| effects.set(effects.get() + 1)));
                    let held = create_memo(move || name.get());
                    view! { <Element tag="h1" text={held} /> }
                }</Loading>
            </Element>
        }
    });

    assert_eq!(page.html(), "<div id=0><p id=2>loading</p></div>");
    assert_eq!(effects.with(Cell::get), 0, "an effect waits until its content is shown");

    name.set(Err(Pending::Failed("refused".to_owned())));
    assert_eq!(page.html(), "<div id=0><p id=3>refused</p></div>");

    name.set(Ok("Groceries".to_owned()));
    assert_eq!(page.html(), "<div id=0><h1 id=1>Groceries</h1></div>");
    assert_eq!(effects.with(Cell::get), 1);

    name.set(Err(Pending::Loading));
    assert_eq!(
        page.html(),
        "<div id=0><h1 id=1>Groceries</h1></div>",
        "once shown, content keeps what it showed while it loads again"
    );
}
