use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{
    AccessGrant, AccessLevel, AccessListing, BlockInfo, HostReply, HostRequest, ShellDialog,
};
use workspace_ui_types::FolderContent;

use super::*;

#[test]
fn the_share_dialog_says_where_each_persons_access_comes_from() {
    let (mut fixture, block) = editor_sized(Some(Vec2::new(900.0, 900.0)));
    let me = fixture.host.account_id();
    let team = Uuid::new_v4();
    let store = fixture.test.store();
    let mut folder = BlockInfo::new(team, FolderContent::CONTENT_TYPE, BlockParent::Root);
    folder.set_name("Team");
    folder.references = vec![block];
    store.add_block(folder);
    let mut shared = BlockInfo::new(block, FolderContent::CONTENT_TYPE, BlockParent::Block(team));
    shared.set_name("Plans");
    store.add_block(shared);
    fixture.test.show_dialog(block, ShellDialog::Share);
    fixture.settle();

    let listing = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|(request, asked)| {
            (asked == HostRequest::ListAccess(block.into_bytes())).then_some(request)
        })
        .expect("opening the share dialog asks who can open the block");
    let person = |name: &str, effective: AccessLevel, source: Uuid| AccessGrant {
        account: Uuid::new_v4().into_bytes(),
        email: format!("{}@example.org", name.to_lowercase()),
        display_name: name.to_owned(),
        administrator: false,
        author: false,
        granted: None,
        inherited: effective,
        effective,
        source: Some(source.into_bytes()),
        waiting: false,
    };
    fixture.test.reply(
        listing,
        HostReply::AccessListed(AccessListing::Listed {
            grants: vec![
                AccessGrant {
                    account: me.into_bytes(),
                    ..person("Me", AccessLevel::Manage, team)
                },
                person("Ada", AccessLevel::Edit, team),
                AccessGrant {
                    granted: Some(AccessLevel::View),
                    inherited: AccessLevel::None,
                    ..person("Grace", AccessLevel::View, block)
                },
                AccessGrant {
                    granted: Some(AccessLevel::Manage),
                    inherited: AccessLevel::View,
                    ..person("Linus", AccessLevel::Manage, block)
                },
            ],
            company: true,
            set_inside: 3,
        }),
    );
    fixture.settle();

    assert!(
        fixture.says("Can edit, from Team"),
        "inherited access names its folder"
    );
    assert!(
        fixture.says("Can view, set here"),
        "a grant on the block says so"
    );
    assert!(
        !fixture.says("Select..."),
        "someone whose access comes from above shows it, not a blank choice"
    );
    assert!(
        fixture.says(
            "The organization's administrators can manage this block. This can't be changed."
        ),
        "an organization's administrators are always listed"
    );
    assert!(
        fixture.says("3 items inside have different access"),
        "a folder warns about the items inside it that set their own access"
    );
    fixture
        .test
        .snapshot("the_share_dialog_says_where_each_persons_access_comes_from");
}
