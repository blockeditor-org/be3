use super::*;

struct NotifyingRoot;

#[component]
impl BeuiRoot for NotifyingRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        let mentioned = Uuid::from_u128(7);
        let send = move || {
            editor
                .host()
                .notify(editor.block_id(), "Lunch at noon?", &[mentioned]);
        };
        view! {
            <Root>
                <Button @test_id={"notify.send"} label="Send" on_click={send} />
            </Root>
        }
    }
}

#[test]
fn an_editor_notifies_its_blocks_members_through_the_host() {
    let block = Uuid::new_v4();
    let mut test = BeuiTest::<NotifyingRoot>::editing(block);
    test.settle();
    assert!(test.take_notifications().is_empty());

    test.click("notify.send");
    test.settle();
    assert_eq!(
        test.take_notifications(),
        [(block, "Lunch at noon?".to_owned(), vec![Uuid::from_u128(7)])]
    );
}
