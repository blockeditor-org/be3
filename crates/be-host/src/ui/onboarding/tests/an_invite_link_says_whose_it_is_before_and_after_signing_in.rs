use super::*;
use crate::ui::{JoiningRow, OrganizationsState, OrganizationsView};

fn organizations(joining: JoiningRow) -> crate::ui::HostUi {
    crate::ui::HostUi {
        organizations: OrganizationsView {
            state: OrganizationsState::Loaded,
            joining: Some(joining),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn an_invite_link_says_whose_it_is_before_and_after_signing_in() {
    let invited = JoiningRow {
        organization: "Acme".to_owned(),
        inviter: "Ada (ada@example.com)".to_owned(),
        problem: String::new(),
        can_join: false,
    };
    // Before signing in: whose it is and into what, read with the link alone.
    let before = invited.clone();
    let document = build(move || {
        let view = HostUiStore::new(organizations(before));
        view! {
            <beui::styled::Root>
                <AccountsScreen view />
            </beui::styled::Root>
        }
    });
    painting_of("an_invite_link_before_signing_in", document, false);
    // Signed in with an account already a member: whose invitation it is, and another account.
    let refused = JoiningRow {
        problem: "You are already a member of that organization. Sign in with another account to accept it.".to_owned(),
        ..invited
    };
    let document = build(move || {
        let view = HostUiStore::new(organizations(refused));
        view! {
            <beui::styled::Root>
                <OrganizationsScreen view />
            </beui::styled::Root>
        }
    });
    painting_of("an_invite_link_for_an_account_already_a_member", document, false);
}
