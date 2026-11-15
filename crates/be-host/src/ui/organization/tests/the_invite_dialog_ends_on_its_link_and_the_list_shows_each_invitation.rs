use super::*;
use crate::ui::{InvitationRow, InvitationsView, InviteView};

#[test]
fn the_invite_dialog_ends_on_its_link_and_the_list_shows_each_invitation() {
    // The link, with its QR code, and that nothing was sent.
    let mut shown = Shown::dialogs(HostUi {
        invite: Some(InviteView {
            organization: "Acme".to_owned(),
            busy: false,
            error: None,
            link: Some(
                "https://api.be3.example/#invite/ABCDEFGHJKMNPQRSTVWXYZ234567ABCDEFGHJKMNP"
                    .to_owned(),
            ),
        }),
        ..Default::default()
    });
    shown.painted("the_invite_dialog_ends_on_its_link");
    // Each invitation's state, whose it is and when: copy and revoke for an open one of the
    // person's own, a fresh link for an expired one.
    let row =
        |id: u128, title: &str, detail: &str, own: bool, open: bool, expired: bool| InvitationRow {
            id: uuid::Uuid::from_u128(id),
            title: title.to_owned(),
            detail: detail.to_owned(),
            link: (own && open).then(|| "https://api.be3.example/#invite/CODE".to_owned()),
            revocable: open,
            renewable: expired,
        };
    let mut shown = Shown::dialogs(HostUi {
        invitations: Some(InvitationsView {
            rows: vec![
                row(
                    1,
                    "Waiting · made by Ada on 2026-11-15",
                    "Not opened yet · expires 2026-11-22",
                    true,
                    true,
                    false,
                ),
                row(
                    2,
                    "Opened · made by Grace on 2026-11-14",
                    "Opened 2026-11-14 · expires 2026-11-21",
                    false,
                    true,
                    false,
                ),
                row(
                    3,
                    "Accepted · made by Ada on 2026-11-10",
                    "By Linus on 2026-11-11",
                    true,
                    false,
                    false,
                ),
                row(
                    4,
                    "Expired · made by Ada on 2026-11-01",
                    "On 2026-11-08",
                    true,
                    false,
                    true,
                ),
                row(
                    5,
                    "Revoked · made by Ada on 2026-10-30",
                    "On 2026-10-31",
                    true,
                    false,
                    false,
                ),
            ],
            busy: false,
            error: None,
        }),
        ..Default::default()
    });
    shown.painted("the_invite_list_shows_each_invitation");
    let first = uuid::Uuid::from_u128(1);
    shown
        .stepped
        .click(beui_host::native::testing::Target::id(format!(
            "host.invitations.revoke.{first}"
        )));
    shown
        .stepped
        .click(beui_host::native::testing::Target::id(format!(
            "host.invitations.renew.{}",
            uuid::Uuid::from_u128(4)
        )));
    let commands = crate::ui::take_commands();
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, UiCommand::RevokeInvitation(link) if *link == first))
    );
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, UiCommand::InviteMember)),
        "an expired invitation makes a fresh link in one step"
    );
}
