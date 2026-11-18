use be_sdk_beui::beui::{Key, Modifiers};
use be_sdk_beui::{AccessGrant, AccessLevel, AccessListing, ShellDialog};
use be_sdk_test::HostCall;

use super::*;

#[test]
fn the_share_dialog_grants_no_knows_it_exists_but_shows_one_held() {
    let (mut fixture, block) = editor_sized(Some(Vec2::new(900.0, 900.0)));
    let me = fixture.host.account_id();
    let kay = Uuid::new_v4();
    let friend = Uuid::new_v4();
    fixture.test.show_dialog(block, ShellDialog::Share);
    fixture.settle();

    let listing = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::ListAccess {
                reply,
                block: asked,
            } if asked == block.into_bytes() => Some(reply),
            _ => None,
        })
        .expect("opening the share dialog asks who can open the block");
    let grant = |account: Uuid, name: &str, granted: Option<AccessLevel>| AccessGrant {
        account: account.into_bytes(),
        email: format!("{}@example.org", name.to_lowercase()),
        display_name: name.to_owned(),
        administrator: false,
        author: false,
        granted,
        inherited: AccessLevel::None,
        effective: granted.unwrap_or(AccessLevel::None),
        source: granted.map(|_| block.into_bytes()),
        waiting: false,
        through_link: false,
        guest: false,
    };
    fixture.test.answer(
        listing,
        Ok(AccessListing {
            grants: vec![
                grant(me, "Me", Some(AccessLevel::Manage)),
                grant(kay, "Kay", Some(AccessLevel::KnowExists)),
                grant(friend, "Friend", None),
            ],
            company: false,
            set_inside: 0,
            levels: Vec::new(),
        }),
    );
    fixture.settle();
    assert!(
        fixture.says("Knows it exists, set here") && !fixture.says("Select..."),
        "a member granted Knows it exists by another route still shows it"
    );

    fixture.test.click("share.query");
    fixture.settle();
    fixture.test.type_text("fri");
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    fixture.click_text("Can edit");
    assert!(
        fixture.says("Can view"),
        "the access to grant opens its choices"
    );
    assert_eq!(
        count_texts(&fixture, "Knows it exists"),
        2,
        "only Kay's row, as its choice and its note, says Knows it exists: the access to grant doesn't offer it"
    );
    fixture
        .test
        .snapshot("the_share_dialog_grants_no_knows_it_exists_but_shows_one_held");
}

fn count_texts(fixture: &Fixture, words: &str) -> usize {
    fn count(document: &Document, id: NodeId, words: &str) -> usize {
        let here = document
            .arena
            .kind_of(id)
            .is_some_and(|node| document.text(node).contains(words));
        usize::from(here)
            + document
                .children(id)
                .into_iter()
                .map(|child| count(document, child, words))
                .sum::<usize>()
    }
    let document = fixture.test.document();
    document
        .root()
        .map_or(0, |root| count(&document, root, words))
}
