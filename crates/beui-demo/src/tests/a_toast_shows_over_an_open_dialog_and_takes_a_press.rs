use beui::unstyled::Edge;

use super::*;

#[test]
fn a_toast_shows_over_an_open_dialog_and_takes_a_press() {
    let _root = beui::reactive::RootScope::new();
    let size = Vec2::new(800.0, 500.0);
    let heard = create_signal(Vec::<String>::new());
    let dismissed = create_signal(0);
    let document = build(move || {
        let area = NodeRef::new();
        let toasts = vec![Toast {
            id: 1,
            title: "Upload finished".to_owned(),
            message: "notes.txt is in the workspace".to_owned(),
            actions: vec![ToastAction {
                key: "open".to_owned(),
                label: "Open".to_owned(),
            }],
            sticky: true,
            ..Toast::default()
        }];
        view! {
            <Root>
                <Frame @node_ref=&area>
                    <List spacing=Space::None>
                        <Dialog
                            open=true
                            title="Share"
                            width={Size::Custom(720.0)}
                            on_dismiss={move || dismissed.update(|dismissed| *dismissed += 1)}
                        >
                            <Text style=TextStyle::PARAGRAPH>
                                "The toast stays bright over the scrim of this dialog."
                            </Text>
                        </Dialog>
                        <Toasts
                            anchor={area}
                            edge=Edge::TopEnd
                            toasts={toasts}
                            on_dismiss={|_: u64| {}}
                            on_action={move |(id, key): (u64, String)| {
                                heard.update(|heard| heard.push(format!("{id} {key}")))
                            }}
                            on_activate={|_: u64| {}}
                        />
                    </List>
                </Frame>
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, size);
    test.snapshot("a_toast_over_a_dialog");

    test.click("toast.1.action.open");
    test.frame(Vec::new());
    assert_eq!(
        heard.get_untracked(),
        ["1 open"],
        "the toast takes the press though a dialog is open beneath it"
    );
    assert_eq!(
        dismissed.get_untracked(),
        0,
        "and the press is not one outside the dialog"
    );
}
