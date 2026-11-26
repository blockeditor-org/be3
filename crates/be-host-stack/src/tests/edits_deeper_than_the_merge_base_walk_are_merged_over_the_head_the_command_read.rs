use super::*;

use be_sdk_api::VersionCommand;

/// Sets the organization to keep every commit of a day, so the server's retention thins away
/// none of the other device's saves, which all land in one minute: the walk reaches them all.
async fn keep_every_commit(harness: &Harness) {
    use be_block::{OrganizationPolicies, OrganizationPoliciesContent, Settings, SettingsContent};
    use be_block_content::Root;

    let peer = harness.outside_peer().await;
    let settings = peer.organization_settings();
    let policies = peer
        .create_with_id::<OrganizationPoliciesContent>(
            Uuid::now_v7(),
            be_block_graph::BlockParent::Block(settings),
        )
        .await
        .expect("the policies are made");
    let mut content = OrganizationPoliciesContent::default();
    content.apply(&OrganizationPolicies::set_history_days(Some(1)));
    peer.save(policies, &content, None)
        .await
        .expect("the policies are saved");
    let head = peer
        .summary(settings)
        .await
        .expect("the settings are read")
        .head;
    let mut held = peer
        .open::<SettingsContent>(settings)
        .await
        .expect("the settings open")
        .unwrap_or_default();
    held.apply(&Settings::set_policy(
        OrganizationPolicies::CONTENT_TYPE,
        policies,
    ));
    peer.save(settings, &held, head)
        .await
        .expect("the settings are saved")
        .published()
        .expect("the settings are published");
}

/// A write of a block that moved walks only `MERGE_BASE_DEPTH` levels of its history to find
/// the base of the merge; past that it merges over the head the command read, which edits made
/// meanwhile were made on.
#[test]
fn edits_deeper_than_the_merge_base_walk_are_merged_over_the_head_the_command_read() {
    let harness = Harness::start();
    harness.connect();
    let runtime = runtime();
    runtime.block_on(keep_every_commit(&harness));
    let versioned = start_versioning("hello\n");
    show_version(
        versioned.checkout,
        be_block_repository::CheckoutContent::CONTENT_TYPE,
    );
    open(versioned.note, be_type_text::TextContent::CONTENT_TYPE);
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
    type_text(versioned.note, 6, "there\n");
    flush_and_send();
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

    close(versioned.note);
    wait_until("closed the note", |shared| {
        !shared.blocks.contains_key(&versioned.note)
    });
    let (reached, held) = tokio::sync::oneshot::channel();
    let (go, gone) = tokio::sync::oneshot::channel();
    crate::hold_next_version_write((reached, gone));
    version(
        versioned.checkout,
        VersionCommand::Switch {
            branch: be_block_repository::MAIN_BRANCH.to_owned(),
        },
    );
    assert_eq!(
        held.blocking_recv(),
        Ok(versioned.note),
        "the switch reached writing the note"
    );
    // Another device saves the note more times than the walk reads, each on the last, adding a
    // line at the top and changing it each time.
    runtime.block_on(async {
        let peer = harness.outside_peer().await;
        let mut head = peer
            .remote_head(versioned.note)
            .await
            .expect("the note's head is read");
        let mut first_save = None;
        for save in 0..=crate::version::MERGE_BASE_DEPTH {
            let first = if save % 2 == 0 { "Oh" } else { "Ah" };
            let text = format!("{first}\nhello\nthere\n");
            head = Some(
                peer.save(versioned.note, &be_type_text::TextBlock::of(&text), head)
                    .await
                    .expect("the edit is saved")
                    .published()
                    .expect("the edit is published"),
            );
            first_save = first_save.or(head);
        }
        let first_save = first_save.expect("the other device saved");
        assert!(
            harness
                .outside_peer()
                .await
                .try_fetch(first_save.hash())
                .await
                .expect("the server answers"),
            "the server kept the oldest of the other device's saves"
        );
    });
    go.send(()).expect("the switch waits");

    wait_until("finished the switch", |shared| {
        settled(shared, versioned.checkout)
            && version_status(shared, versioned.checkout)
                .is_some_and(|status| status.error.is_none())
    });
    let text = runtime.block_on(async {
        let peer = harness.outside_peer().await;
        peer.open::<be_type_text::TextContent>(versioned.note)
            .await
            .expect("the note opens")
            .map(|content| be_type_text::TextBlock::text(&content))
    });
    assert_eq!(
        text.as_deref(),
        Some("Oh\nhello\n"),
        "the line added elsewhere is merged with what the switch wrote"
    );
    // The other device's side reaches its head and a commit a level for `MERGE_BASE_DEPTH`
    // levels, this side at most as many, and the walk goes no further, though their history
    // goes on past it.
    let walked = crate::version::WALKED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let depth = crate::version::MERGE_BASE_DEPTH;
    assert!(
        matches!(
            walked[..],
            [(be_block_commit::MergeBase::Unreached, reached)]
                if reached > depth && reached <= 2 * (depth + 1)
        ),
        "the walk stopped at its depth: {walked:?}"
    );
}
