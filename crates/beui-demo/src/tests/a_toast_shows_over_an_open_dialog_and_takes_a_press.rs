use beui::reactive::{Frame, List, NodeRef, build, create_signal, view};
use beui::styled::{Dialog, Paragraph, Toast, ToastAction, Toasts, use_theme};
use beui::unstyled::Edge;

use super::*;

#[test]
fn a_toast_shows_over_an_open_dialog_and_takes_a_press() {
    let _root = beui::reactive::RootScope::new();
    let size = Vec2::new(800.0, 500.0);
    let (heard, set_heard) = create_signal(Vec::<String>::new());
    let (dismissed, set_dismissed) = create_signal(0);
    let document = build(move || {
        let theme = use_theme();
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
            <Frame @node_ref=&area color={theme.background}>
                <List spacing=0.0>
                    <Dialog
                        open=true
                        title="Share"
                        width=720.0
                        on_dismiss={move || set_dismissed.update(|dismissed| *dismissed += 1)}
                    >
                        <Paragraph
                            content="The toast stays bright over the scrim of this dialog."
                        />
                    </Dialog>
                    <Toasts
                        anchor={area}
                        edge=Edge::TopEnd
                        toasts={toasts}
                        on_dismiss={|_: u64| {}}
                        on_action={move |(id, key): (u64, String)| {
                            set_heard.update(|heard| heard.push(format!("{id} {key}")))
                        }}
                        on_activate={|_: u64| {}}
                    />
                </List>
            </Frame>
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
