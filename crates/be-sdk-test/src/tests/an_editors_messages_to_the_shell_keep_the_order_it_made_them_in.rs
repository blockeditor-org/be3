use be_sdk_api::shell_types::ShellMessage;
use be_sdk_api::{DialogAnswer, ViewChange};
use beui::Vec2;
use beui::styled::Root;

use super::*;

struct OrderRoot;

#[component]
impl BeuiRoot for OrderRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        let host = editor.host();
        host.present(true);
        host.close_dialog(DialogAnswer::Dismissed);
        host.pan_view(Vec2::new(1.0, 0.0));
        host.close_dialog(DialogAnswer::Accepted);
        let acted = host.clone();
        host.act(move || acted.fit_view());
        host.close_dialog(DialogAnswer::Dismissed);
        view! {
            <Root>
                <Text>"Order"</Text>
            </Root>
        }
    }
}

#[test]
fn an_editors_messages_to_the_shell_keep_the_order_it_made_them_in() {
    let mut test = BeuiTest::<OrderRoot>::editing(Uuid::new_v4());
    test.run();

    let told = test.take_shell_messages(|message| match message {
        ShellMessage::Present(_) => Some("present"),
        ShellMessage::CloseDialog(DialogAnswer::Dismissed) => Some("dismissed"),
        ShellMessage::ChangeView(ViewChange::Pan(_)) => Some("view"),
        ShellMessage::ChangeView(ViewChange::Fit) => Some("fit, told inside an act"),
        ShellMessage::CloseDialog(DialogAnswer::Accepted) => Some("accepted"),
        _ => None,
    });
    assert_eq!(
        told,
        [
            "present",
            "dismissed",
            "view",
            "accepted",
            "dismissed",
            "fit, told inside an act"
        ],
        "kinds are not grouped: the order is the order the editor made them in, and a message \
         told inside an act lands after the ones told before it"
    );
}
