use be_sdk_api::shell_types::ShellMessage;
use be_sdk_api::{DialogAnswer, ViewChange};
use beui::Vec2;
use beui::styled::Root;

use super::*;

struct OrderRoot;

const FIRST: Uuid = Uuid::from_u128(1);
const SECOND: Uuid = Uuid::from_u128(2);

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
        host.request_paste();
        host.leave_frame();
        host.request_paste();
        for block in [FIRST, SECOND] {
            host.report_focus(be_sdk_beui::FocusedBlock {
                block_id: Some(block),
                block_type: Uuid::nil(),
                editor: None,
                via: Vec::new(),
            });
        }
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
        ShellMessage::Paste => Some("paste"),
        ShellMessage::LeaveFrame => Some("leave"),
        ShellMessage::Focused(focus) if focus.block_id == Some(FIRST.into_bytes()) => {
            Some("first focus")
        }
        ShellMessage::Focused(focus) if focus.block_id == Some(SECOND.into_bytes()) => {
            Some("second focus")
        }
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
            "leave",
            "paste",
            "second focus",
            "fit, told inside an act"
        ],
        "kinds are not grouped: the order is the order the editor made them in, and a message \
         told inside an act lands after the ones told before it; a paste asked twice is asked \
         once, at the last ask, and only the latest focus is told"
    );
}
