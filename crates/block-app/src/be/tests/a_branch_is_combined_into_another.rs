use super::*;

use block_plugin_api::VersionCommand;

#[test]
fn a_branch_is_combined_into_another() {
    let harness = Harness::start();
    harness.connect();
    let versioned = start_versioning("hello\n");
    version_since(
        versioned.checkout,
        be_block::CheckoutContent::CONTENT_TYPE,
        None,
    );
    hold(versioned.note, text_block_types::TextContent::CONTENT_TYPE);
    wait_until("opened the note", |shared| {
        text_of(shared, versioned.note).is_some()
    });

    version(
        versioned.checkout,
        VersionCommand::CreateBranch {
            name: "draft".into(),
        },
    );
    wait_until("moved onto the new branch", |shared| {
        settled(shared, versioned.checkout)
            && checkout_state(shared, versioned.checkout)
                .is_some_and(|state| state.branch == "draft")
    });
    type_text(versioned.note, 5, " there");
    flush();
    version(
        versioned.checkout,
        VersionCommand::Finish {
            description: "There".into(),
        },
    );
    wait_until("saved the change on the branch", |shared| {
        settled(shared, versioned.checkout)
            && version_status(shared, versioned.checkout)
                .is_some_and(|status| status.log.len() == 2 && status.error.is_none())
    });

    version(
        versioned.checkout,
        VersionCommand::Switch {
            branch: be_block::MAIN_BRANCH.to_owned(),
        },
    );
    wait_until("moved back to main", |shared| {
        settled(shared, versioned.checkout)
            && text_of(shared, versioned.note).as_deref() == Some("hello\n")
            && version_status(shared, versioned.checkout)
                .is_some_and(|status| status.log.len() == 1 && status.changes.is_empty())
    });

    version(
        versioned.checkout,
        VersionCommand::Merge {
            branch: "draft".into(),
        },
    );
    wait_until("combined the branch in", |shared| {
        settled(shared, versioned.checkout)
            && text_of(shared, versioned.note).as_deref() == Some("hello there\n")
            && version_status(shared, versioned.checkout).is_some_and(|status| {
                status.error.is_none()
                    && status
                        .working
                        .as_ref()
                        .is_some_and(|working| working.parents.len() == 2)
            })
    });

    version(
        versioned.checkout,
        VersionCommand::Finish {
            description: "Combined the draft".into(),
        },
    );
    wait_until("saved the combination on main", |shared| {
        settled(shared, versioned.checkout)
            && version_status(shared, versioned.checkout).is_some_and(|status| {
                status.error.is_none()
                    && status.log.len() == 3
                    && status.log[0].message == "Combined the draft"
                    && status.log[0].parents.len() == 2
            })
    });
}
