use super::*;

#[test]
fn the_leave_dialog_quotes_a_long_or_tricky_name_safely() {
    let mut shown = Shown::dialogs(HostUi {
        leaving: Some(crate::ui::LeavingView {
            block: uuid::Uuid::from_u128(8),
            name: "Notes\u{201d} in \u{201c}Private\u{202e}\n and a name that goes on well past what a dialog title should hold"
                .to_owned(),
            within: Some("Team\u{200b}".to_owned()),
        }),
        ..Default::default()
    });
    shown.painted("the_leave_dialog_quotes_a_long_or_tricky_name_safely");
}
