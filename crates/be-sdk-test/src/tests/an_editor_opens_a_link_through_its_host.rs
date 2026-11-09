use beui::styled::Root;

use super::*;

struct LinkRoot;

#[component]
impl BeuiRoot for LinkRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        let opened = create_signal("waiting".to_owned());
        editor.then(
            editor.host().open_url("https://example.test/sign-in"),
            move |answer| {
                opened.set(match answer {
                    Ok(()) => "opened".to_owned(),
                    Err(failure) => failure.to_string(),
                })
            },
        );
        view! {
            <Root>
                <Text @test_id={"link"}>{opened}</Text>
            </Root>
        }
    }
}

#[test]
fn an_editor_opens_a_link_through_its_host() {
    let mut test: BeuiTest<LinkRoot> =
        BeuiTest::new(OwnedEditor::new(EditorHost::default(), Uuid::new_v4()));
    test.settle_until("the link to open", |test| test.label("link") != "waiting");

    assert_eq!(test.label("link"), "opened");
    assert_eq!(
        test.take_opened_urls(),
        vec!["https://example.test/sign-in".to_owned()]
    );
}
