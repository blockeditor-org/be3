use super::*;

use block_plugin_api::VersionCommand;

fn step(shared: &Shared, block: Uuid, starting: &str) -> Option<[u8; 16]> {
    version_status(shared, block)?
        .operations
        .iter()
        .find(|operation| operation.description.starts_with(starting))
        .map(|operation| operation.id)
}

fn saved(shared: &Shared, block: Uuid) -> Option<usize> {
    version_status(shared, block).map(|status| status.log.len())
}

#[test]
fn a_saved_change_is_undone_and_later_edits_stay() {
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

    type_text(versioned.note, 5, " there");
    flush();
    version(
        versioned.checkout,
        VersionCommand::Finish {
            description: "There".into(),
        },
    );
    wait_until("saved the change", |shared| {
        settled(shared, versioned.checkout)
            && saved(shared, versioned.checkout) == Some(2)
            && step(shared, versioned.checkout, "Saved the change").is_some()
    });

    type_text(versioned.note, 0, "Oh, ");
    flush();
    wait_until("listed the later edit", |shared| {
        version_status(shared, versioned.checkout).is_some_and(|status| status.changes.len() == 1)
    });
    let finished = with_shared(|shared| step(shared, versioned.checkout, "Saved the change"))
        .flatten()
        .expect("saving was recorded");
    version(
        versioned.checkout,
        VersionCommand::Undo {
            operation: finished,
        },
    );
    wait_until("undid saving the change", |shared| {
        settled(shared, versioned.checkout)
            && saved(shared, versioned.checkout) == Some(1)
            && step(shared, versioned.checkout, "Undid").is_some()
            && version_status(shared, versioned.checkout).is_some_and(|status| {
                status.error.is_none()
                    && status.changes.len() == 1
                    && status
                        .operations
                        .iter()
                        .any(|operation| operation.id == finished && operation.undone)
            })
    });
    assert_eq!(
        with_shared(|shared| text_of(shared, versioned.note)).flatten(),
        Some("Oh, hello there\n".to_owned()),
        "the edits made since stay"
    );

    let undid = with_shared(|shared| step(shared, versioned.checkout, "Undid"))
        .flatten()
        .expect("undoing was recorded");
    version(
        versioned.checkout,
        VersionCommand::Undo { operation: undid },
    );
    wait_until("brought the saved change back", |shared| {
        settled(shared, versioned.checkout)
            && saved(shared, versioned.checkout) == Some(2)
            && version_status(shared, versioned.checkout).is_some_and(|status| {
                status.error.is_none()
                    && status.log[0].message == "There"
                    && status.changes.len() == 1
            })
    });
    assert_eq!(
        with_shared(|shared| text_of(shared, versioned.note)).flatten(),
        Some("Oh, hello there\n".to_owned())
    );
}
