use beui::styled::Root;

use super::*;

struct ScopesRoot;

#[component]
impl BeuiRoot for ScopesRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        let _ = editor;
        view! {
            <Root>
                <List spacing=Space::Md>
                    <Scoped id="a" />
                    <Scoped id="b" />
                </List>
            </Root>
        }
    }
}

#[component]
fn Scoped(id: &'static str) -> NodeId {
    let area = NodeRef::new();
    action_scope(&area);
    let label = create_signal(format!("Tidy {id}"));
    Action::new(format!("tidy.{id}"), label, || {}).register();
    let text = create_signal(String::new());
    view! {
        <Frame @node_ref=&area>
            <List spacing=Space::Md>
                <TextInput @test_id={format!("{id}.field")} value={text} label="Field" />
                <Button
                    @test_id={format!("{id}.rename")}
                    label="Rename"
                    on_click={move || label.set(format!("Tidy {id} again"))}
                />
            </List>
        </Frame>
    }
}

#[test]
fn an_actions_label_and_where_the_focus_is_reach_the_host() {
    let mut test = BeuiTest::<ScopesRoot>::editing(Uuid::new_v4());
    test.click("a.field");
    test.run();
    assert!(
        test.described_action("tidy.a").live && !test.described_action("tidy.b").live,
        "only the actions around the focus are live"
    );

    test.click("b.field");
    test.run();
    assert!(
        !test.described_action("tidy.a").live && test.described_action("tidy.b").live,
        "moving the focus sends the actions again"
    );

    test.click("a.rename");
    test.run();
    assert_eq!(
        test.described_action("tidy.a").label,
        "Tidy a again",
        "a label that changes is sent again"
    );
}
