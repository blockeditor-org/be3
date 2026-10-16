use block_editor_beui::{Contacts, HostContact, HostRequest, ShellDialog};

use super::*;

fn listings(fixture: &mut Fixture, block: Uuid) -> usize {
    fixture
        .test
        .take_requests()
        .into_iter()
        .filter(|(_, asked)| *asked == HostRequest::ListAccess(block.into_bytes()))
        .count()
}

#[test]
fn the_share_dialog_lists_again_when_a_contact_is_made() {
    let (mut fixture, block) = editor();
    fixture.test.show_dialog(block, ShellDialog::Share);
    fixture.settle();
    assert_eq!(listings(&mut fixture, block), 1);

    fixture.test.set_host_value::<Contacts>(&vec![HostContact {
        account: [7; 16],
        display_name: "Ada".to_owned(),
        email: "ada@example.org".to_owned(),
        verified: true,
    }]);
    fixture.settle();
    assert_eq!(
        listings(&mut fixture, block),
        1,
        "a new contact is listed without anyone asking"
    );
}
